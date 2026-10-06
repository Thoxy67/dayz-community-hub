# Changelog

What changed in each release, newest first. A section written here under
`## x.y.z` before `make publish` becomes that release's notes and the update's
notes; without one, the notes are the commit titles since the last release.

## Unreleased

## 0.5.2 - 2026-10-06

- **DayZ no longer shows as running after opening Mods.** The launcher
  talked to Steam as DayZ and Steam kept the game "running" until the
  launcher closed, so DayZ would not start. Every call to Steam now runs in a
  short-lived process of its own, and launching the game ends one still open.
- **A Statistics page.** The launcher now notes every time DayZ runs: on
  which server or offline map, from when to when (a game started outside
  the launcher counts too). The page shows your time played, sessions,
  average and longest session and streak of days, for 7 days, 30 days, a
  year or all time; a calendar of your days, year by year; the servers and
  maps you play most; the hours of the week you play; and the full history
  of sessions, searchable. Every chart has tooltips, works with the arrow
  keys, and a click on a day, a server or a map lists its sessions. Your
  past joins are carried over (without their length, which was never
  measured). The history travels with the profile export and comes back
  with an import. The page sits under Intel and takes Ctrl+9, which
  Settings had.
- **Ctrl+K finds anything**: a page, an action (refresh, rejoin, update or
  check the mods), a favourite, an installed mod or any server, and Enter
  does it. A Search button in the title bar opens it too.
- **Right-click a server** in the browser, favourites or history for every
  action on it: join, details, favourite, ping, copy the address or a
  `dzch://` link, Direct Connect, exclude the IP. **Right-click a mod** for
  its own (update, repair, link, folders, Workshop, delete).
- **Know before joining whether your mods are ready.** A server's mod count
  is green when you have them all up to date, orange with ↑n when n of yours
  are behind, red with −n when n are missing.
- **The server browser reads faster.** Join buttons are an outline until the
  row is hovered or chosen; the filters in force are spelled out under the
  toolbar, each with its ✕; the header figures are shortcuts (Players and
  Best ping sort, Full and Empty hide those servers, Modded keeps only
  modded ones).
- **? shows every keyboard shortcut**, instead of the cramped line of keys
  in the lists.
- **Mods** sorted by status fold into groups (coming through Steam, updates
  waiting, not checked, up to date), each line says how many of your
  favourites run the mod, and its details list them, or say that none does
  and what deleting it frees. The list keeps its shape while it loads.
- **Direct Connect starts from your servers**, shown as cards until one is
  queried, and an `IP:port` or a `dzch://` link pasted anywhere in the window
  opens there.
- **Offline maps** say what their save weighs and when they were last
  played, and one map's save can be cleared without the others.
- The title bar no longer repeats the launcher update and the mods behind
  (the status bar and the side rail say them); the launch options' command
  line is no longer cut off; the exclude and remove buttons turn red
  anywhere under the pointer, not only on their icon.
- **AUR:** the package is built with makepkg's optimisation flags.

## 0.5.1 - 2026-10-05

- **Share a server with a friend.** Every row in the browser, favourites and
  history has a button (or L) that copies a `dzch://` link. A saved password
  is never put in it.
- **Password servers ask for the password** when joined from the list, from a
  favourite without one, or with Rejoin, and can save it with the favourite.
  Before, the game was launched without it and turned away.
- **BattleMetrics is gone**: its API now needs a paid subscription. Server
  stats come from DayZ Metrics alone, without a key, and show much more: a
  month of players, the busiest hours of the week in your own time, the rank
  trend, how reliably the server restarts on time, and its wipe phase. Looking
  a server up there takes about a second instead of ten or more. The Distance
  fact, which needed BattleMetrics' coordinates, goes with it.
- **Close a hung game.** While DayZ runs, the side rail offers to close it,
  under Proton too.
- **A new setup.** The language comes first; the launcher finds DayZ in any
  Steam library on any drive (or takes the one you pick); mods can download
  through the Steam client, in which case no SteamCMD and no Steam login are
  needed; the last page says how many servers are ready.
- **Switch the mod downloader from the status bar**: SteamCMD or the Steam
  client, one click away.
- **Linux: see how DayZ really starts.** The launch options page shows the
  exact command, the Proton build, its prefix and the launch options you set
  in Steam, taken apart.
- **Controllers.** X and Y do something in every view, X confirms a dialog, a
  second A on a server joins it, the right stick scrolls, L3 rejoins the last
  server and R3 opens a server's details.
- **The About page** shows your system, processor, memory and graphics card
  (with its VRAM) with a button to copy them for a bug report, where the data
  comes from, the keyboard and controller shortcuts, and a tabbed guide.
- **Direct Connect** fits the form and the recent servers on one screen; a
  server row's actions sit in one framed group apart from Join.
- **Windows and Linux fixes:**
  - Steam is found wherever it is installed, Flatpak and Debian's
    /usr/games included; a Flatpak Steam used never to start.
  - On Windows the SteamCMD password and Steam Guard code are submitted (they
    were typed but never sent), re-linking a mod works, and `dzch://` links
    and `.dzch` files open with the portable zip too.
  - Cancel stops SteamCMD instead of leaving it running.
  - The setup finds SteamCMD where the launcher does, pasted paths are
    cleaned of quotes and spaces, and a missing Steam folder no longer stops
    detection.
  - F5 and the browser's right-click menu no longer reload or act in the app.
  - Linux no longer prints a GTK warning at start.
- When a server's mods cannot be read, the launcher asks before joining; the
  ping retries setting now works; News no longer takes the arrow keys from
  the other views; the Stats tab no longer crashes on some servers.

## 0.5.0 - 2026-09-29

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
