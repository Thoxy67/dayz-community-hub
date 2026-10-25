# User guide

How to set up DayZ Community Hub and use each part of it. The launcher looks
and works the same on Windows and Linux; where they differ, this page says so.

- [First launch](#first-launch)
- [Finding a server](#finding-a-server)
- [Joining](#joining)
- [Mods](#mods)
- [Favourites, history, friends and statistics](#favourites-history-friends-and-statistics)
- [Direct connect, links and shortcuts](#direct-connect-links-and-shortcuts)
- [Launch options](#launch-options)
- [Offline mode](#offline-mode)
- [Settings](#settings)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Controller and Steam Deck](#controller-and-steam-deck)
- [Linux notes](#linux-notes)
- [Your data](#your-data)
- [Troubleshooting](#troubleshooting)

## First launch

Steam and DayZ must be installed. The first start opens a short setup wizard
(about a minute):

1. **Welcome.** Coming from another computer? *Import profile* brings your
   settings, favourites and history from an exported profile file.
2. **Where is DayZ?** The launcher searches every Steam library on every
   drive. If it does not find DayZ, pick the Steam library yourself (the folder
   that holds `steamapps`); you can also go on and set it later.
3. **How should mods download?**
   - **Steam client**: Steam downloads them, as it would for the official
     launcher. Steam must be open.
   - **SteamCMD**: works with Steam closed and keeps mods in the launcher's own
     folder; it needs your Steam login once. On Windows the launcher can
     install SteamCMD for you; on Linux, install it with your package manager.

   You can switch later from the status bar or the settings.
4. **Steam account.** Your in-game name (read from Steam when it is running)
   and, for SteamCMD, your Steam user name. The password is never saved:
   SteamCMD asks for it, and for the Steam Guard code, at the first download.
5. **Services** (optional): a [Steam Web API key](https://steamcommunity.com/dev/apikey)
   and your Steam ID. See [why a key helps](#the-steam-web-api-key).
6. **Make it yours**: theme and language.
7. **Ready.** Go to the servers, your favourites or your mods.

## Finding a server

The **Servers** view lists every server the launcher knows of, from two
sources shown in the bar above the list:

- **Public list**: the community list of DayZ servers, about 10,000 of them.
- **Steam**: every server Steam's own master lists, which adds thousands the
  public list lacks. This needs a [Steam Web API key](#the-steam-web-api-key)
  and can be turned off in Settings → Steam & SteamCMD.

Servers are pinged through one shared, paced connection, so the list fills in
over a few seconds without flooding your network.

**Search and filters.** The search box matches names, addresses and maps.
The buttons beside it filter first person, password, BattlEye, modded and
official servers (each: all, only, or none); the map list takes several maps,
and the filter button opens the rest: hide empty or full servers, a highest
ping, the time of day in game (day 06:00–20:00, or night), servers with a
given mod, and **ready to join**: every mod installed and up to date, and the
same game version as yours. Click a column header to sort.

**Details.** Select a server (click it, or `I`) to open its details beside the
list:

- **Overview**: address, share link, map, version, platform, time speed, queue.
- **Players** on it now, **Mods** with which you have, lack or need to update,
  and its **Rules**.
- **Stats** from DayZ Metrics: rank, uptime, peak and average players,
  restarts, the next wipe, and a warning when the player count looks padded
  with bots or fake slots.

A server's row menu (right click, or Menu on a controller) copies its address
or link, opens it in Direct Connect, makes a desktop shortcut, or excludes its
address from the list.

## Joining

Press **Connect** (or `Enter` on a selected row). The launcher then:

1. Offers to **install or update the server's mods** if some are missing or
   behind. You can join without them, but the game may refuse you or crash.
2. Links the server's mods into the game folder.
3. Checks BattlEye on servers that use it, and repairs it when it is broken.
4. Starts Steam if it is not running, and asks it to start DayZ with the
   server's address, its mods, your in-game name and your launch options.

The server goes into your history. *Rejoin* in the side bar (or `Ctrl+L`)
joins the last server again.

## Mods

The **Mods** view lists every mod you have, grouped by state: coming through
Steam, updates waiting, not checked yet, up to date. Each row shows where the
mod comes from, which favourites or played servers use it, whether it is
linked to the game, its size and its last update.

- **Check updates** compares your mods with the Workshop; **Update** (or
  `Ctrl+U` anywhere) updates those behind.
- Tick several (`Space`) to update, link or delete them together.
- **Unused** lists the mods no favourite and no played server needs, to remove
  them in one go. A server's own mods can also be removed from its details.
- **Install mod** adds one by its Workshop id or URL.

**How mods reach the game.** Mods are not copied. Each one gets a link named
`@<workshop id>` in the DayZ folder (a symbolic link on Linux, a junction on
Windows) pointing to where it was downloaded: Steam's Workshop folder
(`steamapps/workshop/content/221100`) with the Steam client, or the launcher's
own folder with SteamCMD.

Switching between the Steam client and SteamCMD: in Steam client mode each
mod is subscribed on your account, and Steam shows you as playing DayZ while
it downloads. In SteamCMD mode, each SteamCMD login disconnects the Steam
client.

## Favourites, history, friends and statistics

- **Favourites**: star a server anywhere (`F`). A favourite can carry a note,
  coloured labels and a group, and its own in-game name and launch arguments
  (for a different character on that server).
- **History**: every server you joined, newest first, with when you last
  played it.
- **Friends**: your Steam friends who play DayZ and the server they are on,
  with a button to join them. Your Steam profile's privacy decides what Steam
  shares.
- **Statistics**: your own play time, by day, server, map and hour of the
  week, from your sessions. The profile can be exported and imported here too.

## Direct connect, links and shortcuts

**Direct connect** joins any server by its address (the port defaults to
2302). *Query* asks the server for its name, players and mods first.

A server's **share link** (`dzch://…`, `L` on a row) opens it in anyone's
launcher, and so does a `.dzch` file: the launcher opens it in Direct Connect,
offers the missing mods and joins. A **desktop shortcut** does the same for one
server.

From a terminal: `dayz-community-hub --connect IP[:PORT]` opens Direct Connect
on that address, and `--reconnect` joins your last server. Only one launcher
runs at a time; a second start hands its link or arguments to the first.

## Launch options

The **Launch options** view sets the DayZ start-up flags: performance (CPU
cores, threads, memory), window mode, start-up (skip the intro, no splash),
and more. *This computer* reads your processor and memory and suggests values;
*Apply* takes them. The command line DayZ will receive is shown below. A
favourite can add its own arguments on top.

## Offline mode

**Offline** plays [DayZ Community Offline Mode](https://github.com/Arkensor/DayZCommunityOfflineMode)
missions alone on your own machine: the full map, no other players, an admin
toolbox.

- **Install / Update** downloads the latest release from GitHub into the
  `Missions` folder of DayZ.
- Each mission (Chernarus+, Livonia, Sakhal…) has **Launch**, its folder, and
  **Clear saves**, which wipes that mission's progress and keeps the mission.
- Tick installed mods on the right to load them with every offline mission.

## Settings

Open them from the side bar or with `Ctrl+,`.

| Page | What it holds |
|---|---|
| Account | In-game name, SteamCMD user name (empty: anonymous where possible), SteamCMD login |
| Steam & SteamCMD | Mod download method, Steam folder, SteamCMD path (and its install on Windows), download folder, Steam's server list |
| API keys | Steam Web API key and Steam ID |
| Location | Your position, detected or typed, for the distance to servers |
| Ping | Which views ping by themselves, timeouts, retries |
| Excluded IPs | Addresses whose servers stay hidden |
| Appearance | Theme (or follow the system, or your own from 12 colours), interface size, seasonal themes and effects, rounded corners |
| Language | English, French, German, Spanish, Russian |
| Window & notifications | Close to the tray; what to do when DayZ starts and stops; system notifications |
| Controller | Controller mode, button labels, on-screen keyboard, size, a controller test, Add to Steam |
| BattlEye | Repair BattlEye |
| Discord | Rich Presence, and whether to name the server |
| Profile data | Export, import, reset |

### The Steam Web API key

The key is optional and free ([get one](https://steamcommunity.com/dev/apikey)). It is stored in your profile and sent only to
Steam. With it, the launcher can:

- list the servers Steam's master knows (thousands more than the public list),
  and Bohemia's official servers;
- check your mods for updates with higher limits;
- show your Steam avatar.

### Seasonal themes

In October the launcher wears **Pumpkin Night** (fog, a spider on the side
bar), and in December **Christmas Night**, or **Powder** if you use a light
theme (falling snow). Your own theme is not changed and comes back after the
season; picking a theme during the season keeps it until the next one. The
effects only move while the launcher is focused and DayZ is not running.
Both can be turned off in Appearance.

## Keyboard shortcuts

`?` shows them all in the launcher, and `Ctrl+K` opens a palette that runs any
action by name.

| Keys | Action |
|---|---|
| `Ctrl+K` | Palette |
| `Ctrl+F` or `/` | Search |
| `Ctrl+R` | Refresh the servers |
| `Ctrl+U` | Update mods that are behind |
| `Ctrl+L` | Rejoin the last server |
| `Ctrl+1` … `Ctrl+0` | Servers, Favourites, History, Friends, Direct connect, Offline, Mods, Launch options, News, Statistics |
| `Ctrl+,` / `F1` | Settings / About |
| `Ctrl+PageUp` / `Ctrl+PageDown` | Previous / next view |
| `Ctrl++` / `Ctrl+−` | Bigger / smaller interface |
| `Esc` | Back |

In a server list: `↑` `↓` `Home` `End` `PageUp` `PageDown` move, `Enter`
joins, `F` favourites, `P` pings, `I` opens the details, `C` copies the
address, `L` copies the share link, `D` opens it in Direct Connect, `Del`
removes a favourite or history entry.

In the mods: `Space` ticks, `U` updates, `M` links or unlinks for the game,
`Del` deletes. In the news: `J` `K` move, `O` opens the article.

## Controller and Steam Deck

A gamepad (Xbox, PlayStation, Steam Deck) drives every view and dialog. The
hint bar at the bottom shows what each button does where you are; the usual
ones: **A** selects or joins, **B** goes back, **X** favourites, **Y** pings,
**Menu** opens a row's actions, **LB/RB** change view, **LT/RT** page, **R3**
opens the details, **L3** rejoins, **View** opens the palette.

While a controller is in use, rows, buttons and text grow, an on-screen
keyboard types into fields, and a built-in file picker replaces the system's.
Settings → Controller chooses when this mode starts, the button labels and the
keyboard. **Add to Steam** adds the launcher to your Steam library, for Game
Mode or Big Picture (close Steam first). The [Steam Deck page](steam-deck.md)
covers the Deck in full.

## Linux notes

- DayZ runs through **Proton**. Choose the Proton version in Steam (DayZ's
  Properties → Compatibility); Launch options → *Launch on Linux* shows the
  one in use, Steam's install type, the Proton prefix and the exact command.
- Native, **Flatpak** and Snap installs of Steam are all found.
- **SteamCMD** comes from your package manager (for example `steamcmd` on Arch
  via the AUR, or Debian/Ubuntu's `steamcmd` package).
- **BattlEye** needs Steam's *Proton BattlEye Runtime*; the status bar shows
  when it is missing, and a click has Steam install it.
- **Tray**: on GNOME the tray icon needs the AppIndicator extension.
- **Controllers** need read access to `/dev/input`.

## Your data

| | Linux | Windows |
|---|---|---|
| Data folder | `~/.local/share/dayz-community-hub/` | `%APPDATA%\dayz-community-hub\` |

In it: `profile.json` (settings, favourites, history, launch options),
`interface.json` (theme, language, sizes), `window.ron` (the window's place),
the server list cache, and SteamCMD's mods (`steamcmd-content/`).

**Moving to another computer**: Settings → Profile data → *Export*, then
*Import profile* in the new launcher's wizard (or the same page).
**Starting over**: *Reset* clears settings, favourites, history and caches and
reopens the wizard; mods on disk are not touched.

## Troubleshooting

**DayZ does not start.** Steam must be running and logged in; the status bar
says when it is not. If Steam is not found, install it or start it once, then
try again.

**Mods do not download.** With SteamCMD: install it (the status bar says when
it is missing), and answer its password or Steam Guard prompt (the status bar
shows *Click to answer*). With the Steam client: Steam must be open.

**BattlEye errors.** The launcher checks BattlEye before each join and offers
to repair it. On Windows, *Repair BattlEye* (Settings → BattlEye) reinstalls it
with administrator rights; it also clears `Documents\DayZ` and
`AppData\Local\DayZ` (settings, key bindings, caches) but keeps offline saves.
If BattlEye is still not ready, run `BattlEye\Install_BattlEye.bat` in the DayZ
folder as administrator. On Linux, install Steam's Proton BattlEye Runtime.

**DayZ hangs.** Close it from the foot of the side bar, or with Steam's
overlay on a controller.

**Few servers.** Without a Steam Web API key only the public list shows; add a
key in Settings → API keys for Steam's full list.

**No tray icon on Linux.** Your desktop needs a tray host (on GNOME, the
AppIndicator extension).

Something else? Open an issue on the
[repository](https://git.thoxy.xyz/thoxy/dayz-community-hub/issues).
