# app

Components that know this app: servers, pings, favourites, the page frame.
`ui/` is the generic kit; what is here is built from it and from the stores,
and is shared by every view that shows the same thing. Before writing a
header, a stat, a server's flags, a ping cell or an empty state in a view,
use the one here; if a view needs a variant, extend the component here
rather than copying it into the feature.

```ts
import { PageHeader, Figure, PingButton, ServerFlags } from "$lib/components/app";
```
