"use client";

import { motion } from "motion/react";
import Link from "next/link";
import { useTheme } from "next-themes";
import { useEffect, useRef } from "react";

export default function HomePage() {
  const { setTheme } = useTheme();

  setTheme("light");

  return (
    <div className="relative h-screen w-full flex md:items-center justify-center ">
      <PlusMinusShader />
      <div className="absolute inset-0 pointer-events-none px-6 md:px-0">
        <div className="max-w-150 mx-auto py-1 mt-6 md:mt-20">
          <motion.img
            src="/logo192.png"
            alt="gitru logo"
            className="absolute size-7 md:-ml-8.5"
            layoutId="logo"
          />
          <div className="flex items-center ml-10 md:ml-0">
            <h1 className="text-2xl font-mono font-[550] pointer-events-auto">
              Gitru
            </h1>
          </div>
          <p className="font-mono mt-2 md:text-justify">
            <span className="pointer-events-auto">
              Gitru is a modern, lightweight, and powerful Git client designed
              to simplify and abstract away the complexity of Git.
            </span>
            <br />
            <br />
            <span className="pointer-events-auto">
              It simplifies complex Git workflows like rebasing and branch
              management, offers a clear visual interface.
            </span>
            <br />
            <br />
            <span className="pointer-events-auto">
              It's not for everyone — Gitru was designed for developers who are
              comfortable with Git concepts but want a cleaner and faster way to
              execute operations.
            </span>
            <br />
            <br />
            <span className="pointer-events-auto">
              As well it brings GitHub features like managing notifications, PR
              reviewing and issue tracking directly into the app.
            </span>
            <br />
            <br />
            <span className="pointer-events-auto">
              It started as a pet project, and now I'm trying to spend more time
              on it. Gitru will be available free and open-source soon. Join the{" "}
              <Link href="/waitlist" className="underline hover:text-primary">
                waitlist
              </Link>{" "}
              for early access and updates!
            </span>
            <br />
            <br />
          </p>

          <div className="flex items-center gap-2 font-mono justify-between group">
            <span className="flex items-center gap-2 pointer-events-auto">
              <Link
                href="/waitlist"
                className="hover:underline cursor-pointer hover:text-primary text-muted-foreground group-hover:text-foreground transition-colors"
              >
                Waitlist
              </Link>
              {/* <p className="text-muted-foreground">·</p>
              <Link
                href="/progress"
                className="hover:underline cursor-pointer hover:text-primary text-muted-foreground group-hover:text-foreground transition-colors"
              >
                Progress
              </Link> */}
              {/* <p className="text-muted-foreground">·</p>
              <Link
                href="/roadmap"
                className="hover:underline cursor-pointer hover:text-primary text-muted-foreground group-hover:text-foreground transition-colors"
              >
                Roadmap
              </Link> */}
            </span>
            <a
              target="_blank"
              rel="noopener noreferrer"
              className="pointer-events-auto hover:underline cursor-pointer hover:text-foreground text-muted-foreground group-hover:text-foreground transition-colors size-4"
              href="https://x.com/ruru_1x"
            >
              <svg fill="none" viewBox="0 0 1200 1227">
                <path
                  fill="currentColor"
                  d="M714.163 519.284 1160.89 0h-105.86L667.137 450.887 357.328 0H0l468.492 681.821L0 1226.37h105.866l409.625-476.152 327.181 476.152H1200L714.137 519.284h.026ZM569.165 687.828l-47.468-67.894-377.686-540.24h162.604l304.797 435.991 47.468 67.894 396.2 566.721H892.476L569.165 687.854v-.026Z"
                />
              </svg>
              {/* <svg viewBox="0 0 256 209" preserveAspectRatio="xMidYMid">
                <path
                  d="M256 25.45c-9.42 4.177-19.542 7-30.166 8.27 10.845-6.5 19.172-16.793 23.093-29.057a105.183 105.183 0 0 1-33.351 12.745C205.995 7.201 192.346.822 177.239.822c-29.006 0-52.523 23.516-52.523 52.52 0 4.117.465 8.125 1.36 11.97-43.65-2.191-82.35-23.1-108.255-54.876-4.52 7.757-7.11 16.78-7.11 26.404 0 18.222 9.273 34.297 23.365 43.716a52.312 52.312 0 0 1-23.79-6.57c-.003.22-.003.44-.003.661 0 25.447 18.104 46.675 42.13 51.5a52.592 52.592 0 0 1-23.718.9c6.683 20.866 26.08 36.05 49.062 36.475-17.975 14.086-40.622 22.483-65.228 22.483-4.24 0-8.42-.249-12.529-.734 23.243 14.902 50.85 23.597 80.51 23.597 96.607 0 149.434-80.031 149.434-149.435 0-2.278-.05-4.543-.152-6.795A106.748 106.748 0 0 0 256 25.45"
                  fill="currentColor"
                />
              </svg> */}
            </a>
          </div>
        </div>
        <div className="h-20" />
      </div>
    </div>
  );
}

function PlusMinusShader() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current!;
    const gl = canvas.getContext("webgl")!;

    function resize() {
      canvas.width = canvas.clientWidth;
      canvas.height = canvas.clientHeight;
      gl.viewport(0, 0, canvas.width, canvas.height);
    }

    resize();
    window.addEventListener("resize", resize);

    const vertexShader = gl.createShader(gl.VERTEX_SHADER)!;
    gl.shaderSource(
      vertexShader,
      `
      attribute vec2 position;
      void main() {
        gl_Position = vec4(position, 0.0, 1.0);
      }
    `,
    );
    gl.compileShader(vertexShader);

    const fragmentShader = gl.createShader(gl.FRAGMENT_SHADER)!;
    gl.shaderSource(
      fragmentShader,
      `
    precision mediump float;

    uniform vec2 u_resolution;
    uniform float u_time;

    float random(vec2 st) {
      return fract(sin(dot(st.xy, vec2(12.9898,78.233))) * 43758.5453);
    }

    float line(vec2 p, vec2 a, vec2 b, float width) {
      vec2 pa = p - a;
      vec2 ba = b - a;
      float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
      float d = length(pa - ba * h);
      return smoothstep(width, width - 0.01, d);
    }

    float drawPlus(vec2 uv) {
      float h = line(uv, vec2(0.2, 0.5), vec2(0.8, 0.5), 0.15);
      float v = line(uv, vec2(0.5, 0.2), vec2(0.5, 0.8), 0.15);
      return max(h, v);
    }

    float drawMinus(vec2 uv) {
      return line(uv, vec2(0.2, 0.5), vec2(0.8, 0.5), 0.15);
    }

    void main() {
      vec2 uv = gl_FragCoord.xy / u_resolution.xy;

      // ~5–10px cells
      vec2 cellCount = u_resolution / 10.0;
      vec2 gridUV = fract(uv * cellCount);
      vec2 id = floor(uv * cellCount);

      float r = random(id);

      // sparse mask (static layout)
      float mask = step(0.92, r);

      // per-cell animation params
      float phase = random(id * 2.1) * 6.2831;      // 0 → 2π
      float speed = mix(0.5, 1.5, random(id * 3.7)); // random speed

      float t = u_time * speed + phase;

      // smooth fade in/out
      float fade = smoothstep(0.35, 0.65, sin(t) * 0.5 + 0.5);

      float shape = 0.0;

      if (mask > 0.0) {
        if (random(id * 1.3) > 0.5) {
          shape = drawPlus(gridUV);
        } else {
          shape = drawMinus(gridUV);
        }
      }

      // subtle whites
      vec3 bg = vec3(1.0);
      vec3 fg = vec3(0.94);

      float finalAlpha = shape * mask * fade;

      vec3 color = mix(bg, fg, finalAlpha);

      gl_FragColor = vec4(color, 1.0);
    }
    `,
    );
    gl.compileShader(fragmentShader);

    const program = gl.createProgram()!;
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);
    gl.useProgram(program);

    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]),
      gl.STATIC_DRAW,
    );

    const position = gl.getAttribLocation(program, "position");
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);

    const uResolution = gl.getUniformLocation(program, "u_resolution");
    const uTime = gl.getUniformLocation(program, "u_time");

    function render(time: number) {
      gl.uniform2f(uResolution, canvas.width, canvas.height);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.uniform1f(uTime, time * 0.001); // seconds
      requestAnimationFrame(render);
    }

    render(0.0001);

    return () => window.removeEventListener("resize", resize);
  }, []);

  return (
    <canvas
      ref={canvasRef}
      style={{
        width: "100%",
        height: "100%",
        display: "block",
        background: "#fff",
      }}
    />
  );
}
