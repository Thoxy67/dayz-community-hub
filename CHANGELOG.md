# Changelog

What changed in each release, newest first. A section written here under
`## x.y.z` before `make publish` becomes that release's notes and the update's
notes; without one, the notes are the commit titles since the last release.

## Unreleased

- **Update by hand once.** Releases are now signed with a new key, so 0.4.1
  and older cannot install this version by themselves: download it from the
  releases page. From this version on, updates install from inside the app on
  Windows and with the AppImage on Linux.
- **A new interface.** Navigation moves to a side rail, every screen is
  redrawn, and a ping reads as a signal meter. Your themes, languages and
  settings carry over; Chernarus (dark) and Survey map (light) are the new
  default themes.
- **The server list no longer freezes the window.** Searching, filtering,
  sorting and counting nine thousand servers now happens in the background;
  the list scrolls without pages and only draws what is on screen.
- **A server's information panel** opens on the right when you select one:
  its map, live ping and players, and tabs for the players online, its mods
  (what you have, what is missing or out of date, with one button to fetch
  them and join), its rules and its BattleMetrics history.
- **Download mods through Steam, if you prefer.** Settings → Steam has a new
  choice: SteamCMD (still the default) or the Steam client, as the DZSA
  launcher does it. With the Steam client, Steam must be running and logged
  in; each mod is subscribed on your account, lands in your own Steam library
  and Steam keeps it updated. The same window shows the progress.
- **The SteamCMD window** shows each mod's progress, speed and time left, and
  SteamCMD's own log live. It can keep running in the background.
- **SteamCMD no longer closes Steam or touches your libraries.** It downloads
  into the launcher's own folder. Steam still disconnects while SteamCMD uses
  your account (one session per account per computer): put it back online
  afterwards, or download through the Steam client instead.
- **Safer.** Links opened from news articles and imported profiles are checked
  before use, every network request gives up after a timeout instead of
  hanging, and a damaged profile no longer stops the app from starting.
- On Steam Deck, closing Steam before a download no longer stops SteamOS's own
  services.
