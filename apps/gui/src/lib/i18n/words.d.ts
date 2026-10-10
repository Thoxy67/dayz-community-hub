// One language's dictionaries, built by the `dzch-words` plugin in vite.config.ts.
declare module "virtual:dzch-words/*" {
  const words: Record<string, import("@intlayer/types/dictionary").Dictionary>;
  export default words;
}
