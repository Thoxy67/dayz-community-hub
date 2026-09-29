<script lang="ts">
  import { dict } from "$lib/i18n";
  import FolderOpen from "~icons/lucide/folder-open";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { pickFile, type FileFilter } from "$lib/ipc/native";

  /**
   * A file or folder path: typed, or chosen with the system's own picker.
   * `title` is the picker's window title.
   */
  let {
    value = $bindable(""),
    id,
    title,
    directory = false,
    filters = [],
    placeholder = "",
    onpick,
  }: {
    value: string;
    id?: string;
    title: string;
    directory?: boolean;
    filters?: FileFilter[];
    placeholder?: string;
    /** Told after the picker returned a path (already written to `value`). */
    onpick?: (path: string) => void;
  } = $props();

  const s = dict("settings");

  async function browse() {
    const path = await pickFile(title, { directory, filters });
    if (!path) return;
    value = path;
    onpick?.(path);
  }
</script>

<Input {id} bind:value {placeholder} spellcheck={false} class="min-w-0 flex-1 font-mono" />
<Button onclick={browse}><FolderOpen class="size-icon-sm" />{$s.browse.value}</Button>
