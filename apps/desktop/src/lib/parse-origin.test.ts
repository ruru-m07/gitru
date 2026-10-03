import { describe, expect, it } from "vitest";
import { parseOrigin } from "./parse-origin";

describe("origin presentation", () => {
  it.each([
    "github.com:qa-fixture/project.git",
    "ssh://github.com/qa-fixture/project.git",
    "git@github.com:qa-fixture/project.git",
  ])("presents %s like the public HTTPS origin", (origin) => {
    const expected = parseOrigin("https://github.com/qa-fixture/project.git");
    expect(parseOrigin(origin)).toEqual({ ...expected, protocol: "ssh" });
    expect(parseOrigin(origin)?.href).toBe(
      "https://github.com/qa-fixture/project",
    );
  });

  it.each([
    "ssh://synthetic-user:synthetic-password@github.com:2222/qa-fixture/project.git?synthetic-token=secret#private",
    "https://synthetic-user:synthetic-password@github.com/qa-fixture/project.git?synthetic-token=secret#private",
    "synthetic-user@github.com:qa-fixture/project.git?synthetic-token=secret#private",
  ])("keeps userinfo and query data out of display and links: %s", (origin) => {
    const parsed = parseOrigin(origin);
    expect(parsed).toMatchObject({
      host: "github.com",
      owner: "qa-fixture",
      repo: "project",
      provider: "github",
      href: "https://github.com/qa-fixture/project",
    });
    expect(JSON.stringify(parsed)).not.toMatch(
      /synthetic-user|synthetic-password|synthetic-token|secret|private/,
    );
  });

  it("preserves subgroup display without resolving custom installation identity", () => {
    expect(
      parseOrigin("ssh://git-work-alias:2222/scm/team/project.git"),
    ).toMatchObject({
      host: "git-work-alias",
      owner: "scm",
      repo: "team/project",
      protocol: "ssh",
      provider: "unknown",
      href: "https://git-work-alias/scm/team/project",
    });
  });

  it.each([
    undefined,
    "file:///tmp/project",
    "https://github.com/qa-fixture/project.git\n",
  ])("ignores missing, local and control-character origins: %s", (origin) =>
    expect(parseOrigin(origin)).toBeUndefined());
});
