export type Appearance = "system" | "light" | "dark";

export interface Settings {
  appearance: Appearance;
}

const KEY = "chime.preferences.v1";
const defaults: Settings = { appearance: "system" };

export function loadSettings(): Settings {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null");
    return {
      appearance: ["system", "light", "dark"].includes(value?.appearance)
        ? value.appearance
        : defaults.appearance,
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
