export type Appearance = "system" | "light" | "dark";
export type Shortcut = "Control+Shift+Space" | "Control+Shift+Period";

export interface Settings {
  appearance: Appearance;
  shortcut: Shortcut;
}

const KEY = "chime.preferences.v1";
const defaults: Settings = { appearance: "system", shortcut: "Control+Shift+Space" };

export function loadSettings(): Settings {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null");
    return {
      appearance: ["system", "light", "dark"].includes(value?.appearance)
        ? value.appearance
        : defaults.appearance,
      shortcut: ["Control+Shift+Space", "Control+Shift+Period"].includes(value?.shortcut)
        ? value.shortcut
        : defaults.shortcut,
    };
  } catch {
    return { ...defaults };
  }
}

export function saveSettings(settings: Settings): boolean {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings));
    return true;
  } catch {
    return false;
  }
}

export function shortcutMatches(event: KeyboardEvent, shortcut: Shortcut) {
  return (
    event.ctrlKey &&
    event.shiftKey &&
    !event.altKey &&
    !event.metaKey &&
    event.code === (shortcut === "Control+Shift+Space" ? "Space" : "Period")
  );
}
