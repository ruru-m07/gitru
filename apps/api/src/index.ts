import { Elysia } from "elysia";

export default new Elysia().get("/", () => "Hello Elysia").listen(3001);
