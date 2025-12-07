import { drizzle } from "drizzle-orm/node-postgres";
import pg from "pg";

const globalForDb = globalThis as unknown as {
  pool: pg.Pool | undefined;
  db: ReturnType<typeof drizzle> | undefined;
};

export const pool =
  globalForDb.pool ??
  (globalForDb.pool = new pg.Pool({
    connectionString: process.env.SUPABASE_POOLING_URL!,
    max: 1,
    ssl: { rejectUnauthorized: false },
  }));

export const db = globalForDb.db ?? (globalForDb.db = drizzle(pool));
