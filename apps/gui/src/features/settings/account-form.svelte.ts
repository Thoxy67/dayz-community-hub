/**
 * The account fields being edited (identity, Steam, API keys, location),
 * held apart from the profile until saved, so the page can say what changed
 * and put it back. Re-read from the profile whenever nothing is pending.
 */
import { profile, type AccountSettings } from "$lib/stores/profile.svelte";

type Fields = {
  player: string;
  steamLogin: string;
  steamRoot: string;
  steamcmdPath: string;
  steamApiKey: string;
  steamId: string;
  /** [longitude, latitude], as the profile keeps it. */
  userLocation: [number, number] | null;
};

/** The profile's location as a point, or none if either half is missing. */
function toPoint(
  v: readonly [number | null, number | null] | null | undefined,
): [number, number] | null {
  return v && v[0] != null && v[1] != null ? [v[0], v[1]] : null;
}

function fromProfile(): Fields {
  const p = profile.data;
  return {
    player: p?.player ?? "",
    steamLogin: p?.steam_login ?? "",
    steamRoot: p?.steam_root ?? "",
    steamcmdPath: p?.steamcmd_path ?? "",
    steamApiKey: p?.steam_api_key ?? "",
    steamId: p?.steam_id ?? "",
    userLocation: toPoint(p?.user_location),
  };
}

const same = (a: Fields, b: Fields) => JSON.stringify(a) === JSON.stringify(b);
const orNull = (s: string) => (s.trim() === "" ? null : s.trim());

class AccountForm {
  f = $state<Fields>(fromProfile());
  #base = $state<Fields>(fromProfile());
  saving = $state(false);

  dirty = $derived(!same(this.f, this.#base));

  /** Follow the profile as long as the player has not started editing. */
  sync() {
    const next = fromProfile();
    if (!this.dirty) this.f = next;
    this.#base = next;
  }

  discard() {
    this.f = fromProfile();
    this.#base = fromProfile();
  }

  async save(overrides: Partial<AccountSettings> = {}) {
    this.saving = true;
    const f = this.f;
    try {
      await profile.saveAccount({
        player: orNull(f.player),
        steamLogin: orNull(f.steamLogin),
        steamRoot: orNull(f.steamRoot),
        steamcmdPath: orNull(f.steamcmdPath),
        steamApiKey: orNull(f.steamApiKey),
        steamId: orNull(f.steamId),
        userLocation: f.userLocation,
        ...overrides,
      });
      this.discard();
    } finally {
      this.saving = false;
    }
  }
}

export const form = new AccountForm();
