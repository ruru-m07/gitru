import "dotenv/config";

import { cors } from "@elysiajs/cors";
import { eq, sql } from "drizzle-orm";
import { Elysia, t } from "elysia";
import { auth } from "./auth";
import { db } from "./db";
import { waitlist } from "./db/waitlist-schema";

const app = new Elysia()
  .use(
    cors({
      origin: process.env.VITE_APP_URL,
      methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
      credentials: true,
      allowedHeaders: ["Content-Type", "Authorization"],
    }),
  )
  .mount(auth.handler)
  .get("/", () => `Hello Elysia`)
  .post(
    "/waitlist",
    async ({ body, set }) => {
      try {
        const [row] = await db
          .insert(waitlist)
          .values({
            name: body.name,
            email: body.email,
          })
          .returning();

        return {
          success: true,
          message: "Successfully registered for the waitlist",
          user: {
            id: row.id,
            name: row.name,
            email: row.email,
          },
        };
      } catch (error: any) {
        if (error.cause?.code === "23505") {
          const existing = await db
            .select()
            .from(waitlist)
            .where(eq(waitlist.email, body.email));

          set.status = 200;

          return {
            success: true,
            message: "You are already on the waitlist",
            user: {
              id: existing[0].id,
              name: existing[0].name,
              email: existing[0].email,
            },
          };
        }

        console.error("waitlist error:", error);

        set.status = 500;
        return {
          success: false,
          message: "Something went wrong",
        };
      }
    },
    {
      body: t.Object({
        name: t.String(),
        email: t.String({ format: "email" }),
      }),
      response: t.Object({
        success: t.Boolean(),
        message: t.String(),
        user: t.Optional(
          t.Object({
            id: t.String(),
            name: t.String(),
            email: t.String(),
          }),
        ),
        joined: t.Optional(t.Number()),
      }),
    },
  )
  .get("/waitlist/count", async () => {
    const [{ count }] = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(waitlist);

    return {
      count: Number(count),
    };
  })
  .listen(process.env.PORT ?? 3001);

console.log(
  `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`,
);

export type App = typeof app;
export default app;
