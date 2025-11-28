import { cors } from "@elysiajs/cors";
import { Elysia, t } from "elysia";

const app = new Elysia()
  .use(cors())
  .get("/", () => "Hello Elysia")
  .post(
    "/user",
    ({ body }) => {
      return {
        success: true,
        message: `Hello ${body.name}!`,
        user: {
          id: crypto.randomUUID(),
          name: body.name,
          email: body.email,
        },
      };
    },
    {
      body: t.Object({
        name: t.String(),
        email: t.String({ format: "email" }),
      }),
      response: t.Object({
        success: t.Boolean(),
        message: t.String(),
        user: t.Object({
          id: t.String(),
          name: t.String(),
          email: t.String(),
        }),
      }),
    },
  )
  .listen(3001);

console.log(
  `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`,
);

export type App = typeof app;
export default app;
