---
title: Troubleshooting
section: Reference
order: 2
---

# Troubleshooting

This page covers the snags you are most likely to hit, with the exact fix for
each. For installation and support questions, the landing page [FAQ](/#faq)
covers additional ground.

## macOS reports Pod is "damaged"

On macOS you may see a dialog that says Pod "is damaged and can't be opened" or
that the developer cannot be verified. The app is not damaged. macOS attaches a
quarantine attribute (`com.apple.quarantine`) to anything downloaded from the
internet, and because the Pod build is not notarized by Apple, Gatekeeper
refuses to launch it until you clear that attribute.

Clear the quarantine from Terminal. Replace the path if you moved the app
somewhere other than `/Applications`:

```sh
xattr -dr com.apple.quarantine /Applications/Pod.app
```

The `-d` flag deletes the attribute and `-r` applies it to everything inside
the bundle. Launch Pod again and it opens normally.

If you prefer not to use Terminal, you can also right-click (or Control-click)
Pod in Finder, choose Open, and confirm Open in the dialog that appears. That
records an explicit exception for this app. The Terminal command is the more
reliable option, because the right-click path is sometimes hidden on newer
macOS versions. In that case, open System Settings, go to Privacy and Security,
scroll to the Security section, and click Open Anyway next to the message about
Pod being blocked.

## Re-authorizing a character

EVE access tokens expire, and they also become invalid if you revoke Pod's
access on the EVE account management site or if CCP rotates the SSO keys. When
that happens, the affected character cannot sync until you sign in again.

Pod shows this state directly on the character card. A red **Fix Permissions**
badge appears on the card whenever the credential is flagged as needing
re-authentication. Click it. Pod starts the EVE SSO sign-in flow in your
browser with the scopes your enabled features require. Log in with the same
character, approve the request, and Pod stores the fresh token. The badge
clears once the new credential validates, and the parked sync jobs resume on
their next scheduled run.

Re-authorizing a corporation works the same way. The card for a corporation
director shows the same badge, and clicking it runs the corporation sign-in
flow with the corporation scopes.

## A character is missing scopes

A character can be authorized but still be missing a scope that a feature
needs. This happens after you turn on a feature that requires access you did
not grant the last time you signed in. For example, enabling Industry adds
scopes for blueprints and industry jobs that an older sign-in never requested.

The symptom is the same red **Fix Permissions** badge on the character card,
because Pod treats a missing scope as a credential that needs
re-authentication. Sync jobs that depend on the missing scope report a blocked
status rather than an error, and the feature stays empty until the scope is
granted.

The fix is also the same. Click **Fix Permissions** on the card. The sign-in
flow Pod launches requests the full scope set for every feature you currently
have enabled, so signing in once grants everything at once. After you approve,
the previously blocked jobs run and the feature populates.

## Sync errors and retries

The sync engine runs in the background and reports its state on the sync chip.
When a job fails, Pod does not stop. It records the failure reason, counts the
job as an error, and schedules a retry. You do not need to do anything for a
transient failure to recover on its own.

How the engine retries depends on why the job failed.

A job that is not ready yet, such as one waiting on a parent record that has
not been written, retries after 3 seconds. When EVE returns a rate-limit
response, Pod waits for the retry window EVE specifies before running that job
again. When EVE signals that the shared error limit is close to exhausted, Pod
pauses dispatching new work entirely until the reset window EVE provides has
passed, which protects your account from a temporary ESI ban.

Authentication failures are treated as permanent. A revoked refresh token, a
missing credential, or a character with no authorizing character does not get
retried in a tight loop. The job is parked and the character is flagged for
re-authentication so the **Fix Permissions** badge appears. See
[Re-authorizing a character](#re-authorizing-a-character) for the fix.

The whole sync engine also has its own restart protection. If the engine
process dies, a supervisor restarts it with an exponential backoff that starts
at 1 second and doubles each time up to a cap of 30 seconds. The backoff
schedule runs 1, 2, 4, 8, 16, then 30 seconds, holding at 30 for any further
restarts. A circuit breaker sits on top of this. If the engine dies 5 times
within a 60 second window, the supervisor stops restarting it rather than
thrashing forever. A restart that survives at least 60 seconds is treated as
healthy and resets the consecutive-failure count back to zero, so an engine
that runs fine for a minute and then dies later gets the full backoff schedule
again rather than tripping the breaker immediately.

If sync stops making progress and stays stopped, the engine has hit the
circuit breaker. Quit Pod and reopen it to start a fresh engine. If it trips
again right away, export your logs (below) so you can see why each restart is
failing.

## Database take-over on a shared database

Pod can keep its database on a shared volume so several installs read the same
data. Only one install holds the database at a time. The holder writes a lease
file and refreshes it on a heartbeat every 10 seconds. A lease counts as stale
after 30 seconds without a heartbeat.

When you open Pod and another install already holds the lease, Pod starts in
read-only mode and shows a banner. The banner names the install that holds the
lease and reads "Open on HOST, close it there, or take over." The clean fix is
to close Pod on the machine named in the banner. The lease releases and your
install acquires it on the next attempt.

If the other machine is off, asleep, or crashed, you can force a take-over.
Click Take over. Pod does not seize the database on the first click. It opens a
second confirmation that tells you how recently the holder was last seen, with
text along the lines of "HOST was last active 2 minutes ago. Taking over
overwrites any unsaved changes it still has open. Continue?" Read the
last-active time before you proceed. If the holder was active seconds ago, it
is probably still running and live, and taking over can discard work it has
open. If it was last active long enough ago to be plausibly dead, click Take
over anyway to force the lease. Click Cancel to back out and stay read-only.

After a successful take-over, Pod pulls the newest copy of the shared database
and switches out of read-only mode.

## Exporting logs

When something is wrong and the cause is not obvious, export your logs and
attach them to a support request. Logs are JSON-formatted records of what the
app and sync engine did. They do not contain secrets such as your tokens.

Open Settings, go to the Storage section, and find the Log path card. Under it
is an **Export logs** row with four range buttons: **Last hour**, **Last 24h**,
**Today**, and **Last 7 days**. Click the range that covers when the problem
happened. Pod gathers the daily log files for that range, filters the boundary
days down to the lines inside the window, and writes a single zip file. The zip
also includes a manifest with the Pod version, your operating system and
architecture, the time range, and the resolved storage paths, which saves a
round trip of questions on a support thread.

## Clean reinstall

A clean reinstall removes Pod's local state and starts over. Do this only when
a normal reinstall has not helped, because it deletes your local database. If
that database lives on a shared volume, or you have re-authorized characters
you do not want to set up again, back up the database file first.

Quit Pod completely, including the background sync engine, before you delete
anything. Then remove Pod's data directory. Pod stores its database as `pod.db`
inside its data directory. The default locations follow each platform's
conventions:

- macOS: `~/Library/Application Support/pod/`, so the database is
  `~/Library/Application Support/pod/pod.db`.
- Linux: `~/.local/share/pod/`, so the database is `~/.local/share/pod/pod.db`.
- Windows: `%APPDATA%\pod\`, so the database is `%APPDATA%\pod\pod.db`.

Logs live in a separate directory:

- macOS: `~/Library/Application Support/pod/logs/`.
- Linux: `~/.local/state/pod/logs/`.
- Windows: `%LOCALAPPDATA%\pod\logs\`.

If you changed the database, log, or cache location in Settings, Pod is using
your custom path instead of the default. The Storage section in Settings shows
the resolved path for each one, so check there before you delete, and remove
the directory it actually points to.

Delete the data directory, then reinstall Pod and launch it. Pod recreates an
empty database, and you add your characters again from the start.
