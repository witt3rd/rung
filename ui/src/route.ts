import { useSyncExternalStore } from "react";

/** Routes live in the hash, so the built app opens from any path with no server rules. */
export type Route =
  | { page: "instances" }
  | { page: "now"; id: string }
  | { page: "turns"; id: string; turn: number | null };

export function parseHash(hash: string): Route {
  const [path, query = ""] = hash.replace(/^#/, "").split("?");
  const parts = path.split("/").filter(Boolean);
  if (parts[0] === "i" && parts[1]) {
    const id = decodeURIComponent(parts[1]);
    if (parts[2] === "turns") {
      const t = Number(new URLSearchParams(query).get("turn"));
      return { page: "turns", id, turn: Number.isInteger(t) && t > 0 ? t : null };
    }
    return { page: "now", id };
  }
  return { page: "instances" };
}

export const href = {
  instances: () => "#/",
  now: (id: string) => `#/i/${encodeURIComponent(id)}`,
  turns: (id: string, turn?: number) => `#/i/${encodeURIComponent(id)}/turns${turn ? `?turn=${turn}` : ""}`,
};

const subscribe = (cb: () => void) => { window.addEventListener("hashchange", cb); return () => window.removeEventListener("hashchange", cb); };
export const useRoute = (): Route => parseHash(useSyncExternalStore(subscribe, () => window.location.hash));
