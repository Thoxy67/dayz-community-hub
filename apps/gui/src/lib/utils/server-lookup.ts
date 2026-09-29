import type { ServerDto } from '$lib/types';

/**
 * Creates a lookup map from servers array.
 * Maps both "ip:query_port" and "ip:game_port" to the server for O(1) lookups.
 *
 * @example
 * let serverByKey = $derived(createServerLookup(servers));
 * const server = serverByKey.get('192.168.1.1:2302');
 */
export function createServerLookup(servers: ServerDto[]): Map<string, ServerDto> {
  const m = new Map<string, ServerDto>();
  for (const s of servers) {
    m.set(`${s.ip}:${s.query_port}`, s);
    m.set(`${s.ip}:${s.game_port}`, s);
  }
  return m;
}

/**
 * Creates a server key string from ip and port.
 */
export function serverKey(ip: string, port: number): string {
  return `${ip}:${port}`;
}

/**
 * Finds a server by ip and port from a lookup map.
 */
export function findServer(lookup: Map<string, ServerDto>, ip: string, port: number): ServerDto | null {
  return lookup.get(serverKey(ip, port)) ?? null;
}
