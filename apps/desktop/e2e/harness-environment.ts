/** Forward OS/runtime inputs only; never read credential or proxy variables. */
const SYSTEM_KEYS = [
  "PATH",
  "HOME",
  "USER",
  "LOGNAME",
  "SHELL",
  "USERPROFILE",
  "APPDATA",
  "LOCALAPPDATA",
  "SystemRoot",
  "WINDIR",
  "SYSTEMDRIVE",
  "ComSpec",
  "PATHEXT",
  "ProgramFiles",
  "ProgramFiles(x86)",
  "ProgramW6432",
  "TEMP",
  "TMP",
  "TMPDIR",
  "LANG",
  "LC_ALL",
  "LANGUAGE",
  "TZ",
  "DISPLAY",
  "WAYLAND_DISPLAY",
  "XAUTHORITY",
  "DBUS_SESSION_BUS_ADDRESS",
  "XDG_RUNTIME_DIR",
  "XDG_DATA_HOME",
  "XDG_CONFIG_HOME",
  "XDG_CACHE_HOME",
] as const;

export function harnessEnvironment(
  inherited: NodeJS.ProcessEnv,
): NodeJS.ProcessEnv {
  const safe: NodeJS.ProcessEnv = {};
  for (const key of SYSTEM_KEYS) {
    const value = inherited[key];
    if (value !== undefined) safe[key] = value;
  }
  return safe;
}
