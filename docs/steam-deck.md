# The launcher on a Steam Deck

The Linux release is an AppImage that runs on SteamOS as it is: it carries its
own type 2 runtime (no libfuse2), is linked against glibc 2.35 and vendors
OpenSSL, so nothing has to be installed or unlocked (no developer mode, no
`steamos-readonly disable`).

## Install (Desktop Mode)

1. Switch to Desktop Mode (Steam button, Power, Switch to Desktop).
2. Download `dayz-community-hub-vX.Y.Z-x86_64.AppImage` from the releases page
   and move it where it can stay, for example `~/Applications/`. The updater
   replaces that file in place, so a shortcut to it keeps working across
   releases.
3. Make it executable: right-click, Properties, Permissions, "Is executable"
   (or `chmod +x ~/Applications/dayz-community-hub-*.AppImage`).
4. Start it once from Dolphin to go through the setup.

## Add to Steam

Settings, Controller, **Add to Steam** puts the launcher in the Steam library
as a non-Steam game, so that Game Mode can start it with Steam Input
configuring the controls. The command palette has it too (`steam.add_shortcut`).

- **Close Steam first.** Steam reads `userdata/<account>/config/shortcuts.vdf`
  when it starts and writes its own copy back when its list changes and when it
  exits: a change made while it runs would be lost, or would overwrite the
  player's own changes. The launcher checks and refuses while Steam runs. In
  Desktop Mode, quit Steam from its tray icon (Steam, Exit), click the button,
  then start Steam again. In Game Mode, where Steam never closes, the button
  is disabled and says to switch to Desktop Mode.
- What is written: the program (the AppImage itself, from `$APPIMAGE`, not its
  temporary mount), its folder, the launcher's own flags (`--data-dir`,
  `--instance`; never `--control` or `--mock`), and the icon, copied to
  `<data folder>/steam-icon.png`. A new entry's id is the one Steam derives
  itself (CRC-32 of the quoted program and the name, top bit set), so art in
  `config/grid/` and `steam://rungameid/` links match what Steam would make.
  On Windows the launch options follow the Windows command line's quoting
  (backslashes stay single, so `\\server\share` paths work).
- Every other entry is kept as it was, byte for byte. The first time, the old
  file is kept as `shortcuts.vdf.dzch-backup`. Clicking again updates the entry
  (same program, or the same name once the AppImage was moved or renamed)
  instead of adding a second one. It keeps the entry's id, so artwork set on
  it stays, and what the player changed in Steam (hidden, tags, overlay, last
  played, an icon of their own). A file the launcher
  cannot read whole is left alone and the button says why.
- The account is the one Steam logged in last (`config/loginusers.vdf`), else
  the only one under `userdata/`. With several accounts and no record, nothing
  is written.
- No grid, hero or logo art is written: Steam shows the icon and its default
  tile. Steam's own "Manage, Set custom artwork" works on the entry as usual.

## Game Mode

The launcher knows it runs on a Deck from the firmware (`board_vendor` Valve,
`product_name` Jupiter or Galileo under `/sys/class/dmi/id`) or from Steam's
`SteamDeck=1`, and that it runs in Game Mode from gamescope
(`XDG_CURRENT_DESKTOP=gamescope` or `GAMESCOPE_WAYLAND_DISPLAY`). Under Game
Mode:

- The window opens full screen (whatever size the last desktop session
  saved); the title bar keeps only its close button, and
  the window has no resize edges (gamescope has no window manager).
- Closing quits: there is no tray to hide to.
- Links open in Steam's own browser (`steam://openurl/…`), the only one that
  can show over gamescope. "Open folder" buttons are hidden: there is no file
  manager; neither is Offline's "Explore".
- The controller drives the window from the start, with the Deck's button
  labels even though Steam Input hands the launcher a virtual Xbox pad.
  Text fields open Steam's on-screen keyboard.
- A touch on the screen does not switch the window to the mouse layout, and
  hit areas grow while fingers are used.
- DayZ starts with the launcher's Steam session variables removed
  (`SteamGameId`, `SteamOverlayGameId`, `SteamAppId` and Steam's overlay in
  `LD_PRELOAD`), so Steam starts it as DayZ (app 221100) rather than as part of
  the launcher's shortcut. Only the started programs are affected; the
  launcher's own environment is unchanged.

On a Deck, a new profile and the first-start setup pick the Steam client
(Steamworks) for mods, which also works in Game Mode. Switching to SteamCMD,
in the setup or in Settings, asks first: it needs a login and Steam Guard code
of its own.

## Not tested on a device

These were built and unit-tested on a desktop (the VDF reader and writer, the
id, the detection, the environment scrub), not on a Deck. To check on one:

- The Steamworks downloader in Game Mode.
- Joining from the shortcut: `-applaunch` from inside a running non-Steam game,
  and gamescope switching to DayZ and back.
- The updater's restart under Steam's reaper.
- winit's backend under gamescope (Wayland or Xwayland), and the on-screen
  keyboard's text reaching the fields.
- gamescope's focus events (the controller is dropped while the window is
  unfocused; a compositor that never says keeps it on).
- That Steam picks the new shortcut and its icon after a restart, on SteamOS's
  current client.
