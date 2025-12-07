import "dotenv/config";
import { defineConfig } from "drizzle-kit";

if (!process.env.SUPABASE_POOLING_URL) {
  throw new Error("SUPABASE_POOLING_URL is not set in the .env file");
}

export default defineConfig({
  schema: ["./src/db/auth-schema.ts", "./src/db/waitlist-schema.ts"],
  out: "./drizzle",
  dialect: "postgresql",
  dbCredentials: {
    url: process.env.SUPABASE_POOLING_URL,
  },
});
