use crate::{
  clients::{
    Error,
    esi::models::character::{Planet, PlanetPin},
  },
  store::{
    model::{
      CharacterPlanet, CharacterPlanetLink, CharacterPlanetPin, CharacterPlanetPinContent, CharacterPlanetRoute,
    },
    repo::{character, colonies},
  },
  sync::{job::JobCtx, outcome::Outcome, subject::Subject},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character planets job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }

  let client = ctx.esi.character_authenticated(grant);
  let colony_list = client.planets().await?;

  let mut planets = Vec::with_capacity(colony_list.len());
  let mut pins = Vec::new();
  let mut contents = Vec::new();
  let mut routes = Vec::new();
  let mut links = Vec::new();

  for colony in &colony_list {
    let planet_id = colony.planet_id;
    let detail = client.planet_detail(planet_id).await?;
    planets.push(to_planet(character_id, colony));

    for pin in &detail.pins {
      pins.push(to_pin(character_id, planet_id, pin));
      for content in &pin.contents {
        contents.push(CharacterPlanetPinContent {
          character_id,
          amount: content.amount.unwrap_or_default(),
          pin_id: pin.pin_id,
          type_id: content.type_id.map(i64::from).unwrap_or_default(),
        });
      }
    }

    for route in &detail.routes {
      routes.push(CharacterPlanetRoute {
        character_id,
        content_type_id: route.content_type_id.map(i64::from).unwrap_or_default(),
        destination_pin_id: route.destination_pin_id.unwrap_or_default(),
        planet_id,
        quantity: route.quantity.unwrap_or_default(),
        route_id: route.route_id,
        source_pin_id: route.source_pin_id.unwrap_or_default(),
      });
    }

    for link in &detail.links {
      links.push(CharacterPlanetLink {
        character_id,
        destination_pin_id: link.destination_pin_id.unwrap_or_default(),
        link_level: link.link_level.map(i64::from).unwrap_or_default(),
        planet_id,
        source_pin_id: link.source_pin_id.unwrap_or_default(),
      });
    }
  }

  colonies::replace_for_character(ctx.db, character_id, &planets, &pins, &contents, &routes, &links).await?;
  Ok(Outcome::from_rows(planets.len()))
}

fn to_planet(character_id: i64, planet: &Planet) -> CharacterPlanet {
  CharacterPlanet {
    character_id,
    last_update: planet.last_update.clone().unwrap_or_default(),
    num_pins: planet.num_pins.map(i64::from).unwrap_or_default(),
    planet_id: planet.planet_id,
    planet_type: planet.planet_type.clone().unwrap_or_default(),
    solar_system_id: 0,
    upgrade_level: planet.upgrade_level.map(i64::from).unwrap_or_default(),
  }
}

fn to_pin(character_id: i64, planet_id: i64, pin: &PlanetPin) -> CharacterPlanetPin {
  let extractor = pin.extractor_details.as_ref();
  CharacterPlanetPin {
    character_id,
    cycle_time: extractor.and_then(|detail| detail.cycle_time).map(i64::from),
    expiry_time: None,
    head_radius: extractor.and_then(|detail| detail.head_radius),
    install_time: None,
    last_cycle_start: None,
    latitude: 0.0,
    longitude: 0.0,
    pin_id: pin.pin_id,
    planet_id,
    product_type_id: extractor.and_then(|detail| detail.product_type_id).map(i64::from),
    qty_per_cycle: extractor.and_then(|detail| detail.qty_per_cycle).map(i64::from),
    schematic_id: pin
      .factory_details
      .as_ref()
      .and_then(|detail| detail.schematic_id)
      .map(i64::from),
    type_id: i64::from(pin.type_id),
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{self, images},
    sync::job::{JobKey, JobKind},
  };

  async fn seed_character(db: &store::Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
    let corp_id = 98_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: Option<&'a Grant>,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterPlanets, Subject::Character(character_id)),
      grant,
      sso: None,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::repo::colonies;

    #[tokio::test]
    async fn it_persists_a_colony_layout_across_every_table() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/planets/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          { "planet_id": 40000001, "planet_type": "barren", "upgrade_level": 5, "num_pins": 3,
            "last_update": "2026-07-13T12:00:00Z" },
        ])))
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/42/planets/40000001/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "pins": [
            { "pin_id": 1001, "type_id": 2848,
              "extractor_details": { "cycle_time": 3600, "head_radius": 0.5, "product_type_id": 2268, "qty_per_cycle": 1500 } },
            { "pin_id": 1002, "type_id": 2541, "factory_details": { "schematic_id": 127 },
              "contents": [{ "type_id": 2268, "amount": 500 }] }
          ],
          "routes": [
            { "route_id": 700, "source_pin_id": 1001, "destination_pin_id": 1002, "content_type_id": 2268, "quantity": 3000 }
          ],
          "links": [
            { "source_pin_id": 1001, "destination_pin_id": 1002, "link_level": 1 }
          ]
        })))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      run(&ctx).await.unwrap();

      let planets = colonies::list_planets_for_character(&db, 42).await.unwrap();
      assert_eq!(planets.len(), 1);
      assert_eq!(planets[0].planet_id(), 40_000_001);
      assert_eq!(planets[0].planet_type(), "barren");
      assert_eq!(colonies::list_pins_for_character(&db, 42).await.unwrap().len(), 2);
      assert_eq!(
        colonies::list_pin_contents_for_character(&db, 42).await.unwrap().len(),
        1
      );
      assert_eq!(colonies::list_routes_for_character(&db, 42).await.unwrap().len(), 1);
      assert_eq!(colonies::list_links_for_character(&db, 42).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_errors_when_the_grant_is_missing() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, None, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_returns_not_ready_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/planets/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", 42);
      let ctx = ctx_with_grant(&db, &esi, &image, &image_store, Some(&grant), 42);

      let result = run(&ctx).await;

      assert!(matches!(result, Err(Error::NotReady)));
    }
  }
}
