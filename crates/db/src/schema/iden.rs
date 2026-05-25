//! SeaORM `DeriveIden` enums for every table and column used in migrations.

use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
pub enum AbyssalItems {
  CharacterId,
  DogmaAttributes,
  ItemId,
  MutaPriceIsk,
  MutaPriceSynced,
  MutatorTypeId,
  SourceTypeId,
  SyncedAt,
  Table,
  TypeId,
}

#[derive(DeriveIden)]
pub enum AbyssalSourceTypes {
  SourceTypeId,
  Table,
}

#[derive(DeriveIden)]
pub enum AbyssalModuleStats {
  AbyssalTypeId,
  AttributeId,
  Id,
  MaxMult,
  MinMult,
  Table,
}

#[derive(DeriveIden)]
pub enum AssetSyncState {
  CacheExpiresAt,
  LastSyncedAt,
  OwnerId,
  OwnerType,
  Table,
}

#[derive(DeriveIden)]
pub enum Bloodlines {
  Charisma,
  CorporationId,
  Description,
  Id,
  Intelligence,
  Memory,
  Name,
  Perception,
  RaceId,
  ShipItemTypeId,
  Table,
  WillPower,
}

#[derive(DeriveIden)]
pub enum CharacterAssets {
  CharacterId,
  IsActiveShip,
  IsBlueprintCopy,
  IsSingleton,
  ItemId,
  LocationFlag,
  LocationId,
  LocationType,
  Quantity,
  ShipName,
  Table,
  TypeId,
}

#[derive(DeriveIden)]
pub enum CorporationAssets {
  CorporationId,
  IsBlueprintCopy,
  IsSingleton,
  ItemId,
  LocationFlag,
  LocationId,
  LocationType,
  Quantity,
  Table,
  TypeId,
}

#[derive(DeriveIden)]
pub enum CharacterCloneImplants {
  AttributeBonus,
  CloneId,
  Id,
  Name,
  Slot,
  Table,
  TypeId,
}

#[derive(DeriveIden)]
pub enum CharacterClones {
  CharacterId,
  Id,
  InstalledAt,
  IsActive,
  LocationId,
  Name,
  RegionName,
  StationName,
  SyncedAt,
  SystemId,
  Table,
}

#[derive(DeriveIden)]
pub enum CharacterContracts {
  AcceptorId,
  AssigneeId,
  CharacterId,
  Collateral,
  ContractId,
  ContractType,
  DateExpired,
  DateIssued,
  Id,
  IssuerId,
  Price,
  StartLocationId,
  Status,
  Table,
  Title,
}

#[derive(DeriveIden)]
pub enum CharacterContactLabels {
  CharacterId,
  Id,
  LabelId,
  LabelName,
  Table,
}

#[derive(DeriveIden)]
pub enum CharacterContacts {
  CharacterId,
  ContactId,
  ContactName,
  ContactType,
  Id,
  IsWatchlist,
  LabelIds,
  Standing,
  SyncedAt,
  Table,
}

#[derive(DeriveIden)]
pub enum CharacterKillmails {
  AttackerCount,
  CharacterId,
  FinalBlow,
  Id,
  IsKill,
  KillHash,
  KillTime,
  KillmailId,
  ShipName,
  ShipTypeId,
  SyncedAt,
  SystemId,
  SystemName,
  SystemSec,
  Table,
  ValueIsk,
  VictimCorpName,
  VictimName,
}

#[derive(DeriveIden)]
pub enum CharacterNotifications {
  CharacterId,
  Id,
  IsRead,
  NotificationId,
  NotifType,
  SenderId,
  SenderType,
  SyncedAt,
  Table,
  Text,
  Timestamp,
}

#[derive(DeriveIden)]
pub enum CharacterSkills {
  ActiveLevel,
  CharacterId,
  IsActiveTraining,
  SkillId,
  Skillpoints,
  Table,
  TrainedLevel,
  TrainingEndTime,
  TrainingStartSp,
  TrainingStartTime,
}

#[derive(DeriveIden)]
pub enum CharacterStandings {
  CharacterId,
  EffectiveStanding,
  FromId,
  FromName,
  FromType,
  Id,
  RawStanding,
  SyncedAt,
  Table,
}

#[derive(DeriveIden)]
pub enum EntityTags {
  EntityId,
  EntityType,
  Table,
  TagId,
}

#[derive(DeriveIden)]
pub enum Characters {
  AccessToken,
  Charisma,
  CorpId,
  CorpName,
  GrantedScopes,
  Id,
  Intelligence,
  IskBalance,
  LocationDocked,
  LocationName,
  Memory,
  Name,
  Perception,
  PortraitTone,
  RefreshToken,
  SortOrder,
  Table,
  TokenExpiresAt,
  Willpower,
}

#[derive(DeriveIden)]
pub enum Constellations {
  Id,
  Name,
  PositionX,
  PositionY,
  PositionZ,
  RegionId,
  Table,
}

#[derive(DeriveIden)]
pub enum DogmaAttributes {
  AttributeId,
  DefaultValue,
  Description,
  DisplayName,
  HighIsGood,
  IconId,
  Id,
  Name,
  Published,
  Stackable,
  Table,
  UnitId,
}

#[derive(DeriveIden)]
pub enum Factions {
  Description,
  Id,
  IsUnique,
  Name,
  SizeFactor,
  SolarSystemId,
  Table,
}

#[derive(DeriveIden)]
pub enum ItemCategories {
  Id,
  Name,
  Published,
  Table,
}

#[derive(DeriveIden)]
pub enum ItemGroups {
  Id,
  ItemCategoryId,
  Name,
  Published,
  Table,
}

#[derive(DeriveIden)]
pub enum ItemTypes {
  Capacity,
  Description,
  DogmaAttributes,
  DogmaEffects,
  GraphicId,
  IconId,
  Id,
  IsAbyssal,
  ItemGroupId,
  MarketGroupId,
  Mass,
  Name,
  PackagedVolume,
  PortionSize,
  Published,
  Radius,
  Table,
  Volume,
}

#[derive(DeriveIden)]
pub enum MarketGroups {
  Description,
  Id,
  Name,
  ParentMarketGroupId,
  Table,
}

#[derive(DeriveIden)]
pub enum Planets {
  Id,
  ItemTypeId,
  Name,
  PositionX,
  PositionY,
  PositionZ,
  SolarSystemId,
  Table,
}

#[derive(DeriveIden)]
pub enum Races {
  AllianceId,
  Description,
  Id,
  Name,
  Table,
}

#[derive(DeriveIden)]
pub enum Regions {
  Description,
  Id,
  Name,
  Table,
}

#[derive(DeriveIden)]
pub enum SolarSystems {
  ConstellationId,
  Id,
  Name,
  PositionX,
  PositionY,
  PositionZ,
  SecurityClass,
  SecurityStatus,
  StarId,
  Table,
}

#[derive(DeriveIden)]
pub enum Stars {
  Age,
  Id,
  ItemTypeId,
  Luminosity,
  Name,
  Radius,
  SolarSystemId,
  SpectralClass,
  Table,
  Temperature,
}

#[derive(DeriveIden)]
pub enum Stargates {
  DestinationSolarSystemId,
  DestinationStargateId,
  Id,
  ItemTypeId,
  Name,
  PositionX,
  PositionY,
  PositionZ,
  SolarSystemId,
  Table,
}

#[derive(DeriveIden)]
pub enum Stations {
  Id,
  ItemTypeId,
  MaxDockableShipVolume,
  Name,
  OfficeRentalCost,
  OwnerId,
  PositionX,
  PositionY,
  PositionZ,
  RaceId,
  ReprocessingEfficiency,
  ReprocessingStationsTake,
  Services,
  SolarSystemId,
  Table,
}

#[derive(DeriveIden)]
pub enum Tags {
  Color,
  Id,
  Name,
  SortOrder,
  Table,
}

#[derive(DeriveIden)]
pub enum WalletJournalEntries {
  Id,
  CharacterId,
  EntryId,
  RefType,
  Amount,
  Balance,
  Date,
  Description,
  FirstPartyId,
  SecondPartyId,
  Table,
}

#[derive(DeriveIden)]
pub enum WalletTransactions {
  Id,
  CharacterId,
  TransactionId,
  TypeId,
  Quantity,
  UnitPrice,
  IsBuy,
  Date,
  LocationId,
  ClientId,
  Table,
}

#[derive(DeriveIden)]
pub enum MailHeaders {
  Body,
  CharacterId,
  FromId,
  Id,
  IsRead,
  MailId,
  Preview,
  RecipientsDisplay,
  Subject,
  Table,
  Timestamp,
}

#[derive(DeriveIden)]
pub enum SnoozedMails {
  Id,
  CharacterId,
  MailId,
  SnoozeUntil,
  Table,
}

#[derive(DeriveIden)]
pub enum Corporations {
  AccessToken,
  AllianceId,
  AllianceName,
  AuthCharacterId,
  CeoCharacterId,
  DateFounded,
  Description,
  FactionId,
  HomeStationId,
  IconData,
  Id,
  MemberCount,
  Name,
  RefreshToken,
  Scopes,
  Shares,
  Table,
  TaxRate,
  Ticker,
  TokenExpiresAt,
  Url,
  WarEligible,
}

#[derive(DeriveIden)]
pub enum TypeIcons {
  Data,
  Table,
  TypeId,
  Variant,
}

#[derive(DeriveIden)]
pub enum SkillPlans {
  CharacterId,
  CreatedAt,
  Id,
  ImplantSet,
  Name,
  RemapJson,
  Table,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum SkillPlanEntries {
  Auto,
  Id,
  Note,
  PlanId,
  Position,
  Priority,
  SkillName,
  Table,
  ToLevel,
}

#[derive(DeriveIden)]
pub enum Certificates {
  Description,
  Grade,
  Id,
  Name,
  SkillsJson,
  Table,
}

#[derive(DeriveIden)]
pub enum ShipMasteryCerts {
  CertIdsJson,
  MasteryLevel,
  ShipId,
  Table,
}

#[derive(DeriveIden)]
pub enum TypePrices {
  AdjustedPrice,
  FetchedAt,
  Id,
  Price,
  Table,
  TypeId,
}

#[derive(DeriveIden)]
pub enum TypePriceHistories {
  Avg,
  Close,
  Date,
  High,
  Id,
  Low,
  Open,
  SampleCount,
  Table,
  TypeId,
}

#[derive(DeriveIden)]
pub enum Stockpiles {
  CharacterId,
  Id,
  LocationId,
  Name,
  Table,
}

#[derive(DeriveIden)]
pub enum StructureCache {
  Id,
  Name,
  SolarSystemId,
  Table,
}

#[derive(DeriveIden)]
pub enum StockpileItems {
  Id,
  StockpileId,
  Table,
  TargetQuantity,
  TypeId,
}
