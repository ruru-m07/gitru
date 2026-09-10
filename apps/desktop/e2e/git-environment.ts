const GIT_ENVIRONMENT_VARIABLE = /^GIT_/i;

const controlledGitEnvironment = (globalConfig: string) => ({
  GIT_CONFIG_GLOBAL: globalConfig,
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_TERMINAL_PROMPT: "0",
});

export function isolatedGitEnvironment(
  globalConfig: string,
  overrides: NodeJS.ProcessEnv = {},
): NodeJS.ProcessEnv {
  const environment = Object.fromEntries(
    Object.entries(process.env).filter(
      ([name]) => !GIT_ENVIRONMENT_VARIABLE.test(name),
    ),
  );

  return {
    ...environment,
    ...overrides,
    ...controlledGitEnvironment(globalConfig),
  };
}

export function isolateCurrentProcessGitEnvironment(
  globalConfig: string,
): void {
  for (const name of Object.keys(process.env)) {
    if (GIT_ENVIRONMENT_VARIABLE.test(name)) {
      delete process.env[name];
    }
  }

  Object.assign(process.env, controlledGitEnvironment(globalConfig));
}
