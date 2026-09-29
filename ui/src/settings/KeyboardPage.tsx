import { ShortcutTable } from "../shell/KeyboardHelp";

/** Settings → Keyboard (004 FR-026): the same list as the `?` overlay. */
export default function KeyboardPage() {
  return (
    <section class="settings-page" aria-labelledby="keyboard-title">
      <h2 id="keyboard-title">Keyboard</h2>
      <p class="lede">
        Press <kbd>?</kbd> anywhere to see these. Sequences like <kbd>g s</kbd> are two keys pressed
        one after the other.
      </p>
      <ShortcutTable />
    </section>
  );
}
