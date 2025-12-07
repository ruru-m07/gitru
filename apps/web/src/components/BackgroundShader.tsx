import { useEffect, useRef } from "react";
import fragmentShaderSource from "./fragment-shader.frag?raw";

const MAX_RIPPLES = 8;

const BackgroundShader = () => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const gl = canvas.getContext("webgl2");
    if (!gl) {
      console.warn("WebGL2 not available");
      return;
    }

    const vertexShaderSource = `
      attribute vec2 a_position;
      void main() {
        gl_Position = vec4(a_position, 0.0, 1.0);
      }
    `;

    function createShader(type: number, source: string) {
      if (!gl) return null;
      const s = gl.createShader(type)!;
      gl.shaderSource(s, source);
      gl.compileShader(s);
      const ok = gl.getShaderParameter(s, gl.COMPILE_STATUS);
      if (!ok) {
        const log = gl.getShaderInfoLog(s);
        console.error("Shader compile error:", log);
      }
      return s;
    }

    const vs = createShader(gl.VERTEX_SHADER, vertexShaderSource);
    const fs = createShader(gl.FRAGMENT_SHADER, fragmentShaderSource);
    if (!vs || !fs) return;

    const program = gl.createProgram()!;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error("Program link error:", gl.getProgramInfoLog(program));
    }
    gl.useProgram(program);

    // screen quad
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
      gl.STATIC_DRAW,
    );
    const posLoc = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

    // uniforms
    const resLoc = gl.getUniformLocation(program, "u_resolution");
    const timeLoc = gl.getUniformLocation(program, "u_time");
    const clickLoc = gl.getUniformLocation(program, "u_clicks");
    const clickTimeLoc = gl.getUniformLocation(program, "u_clickTimes");

    const glyphTexLoc = gl.getUniformLocation(program, "u_glyphs");
    const glyphColsLoc = gl.getUniformLocation(program, "u_glyphCols");
    const glyphRowsLoc = gl.getUniformLocation(program, "u_glyphRows");

    // --- glyph atlas setup (offscreen canvas) ---
    const baseChars = "abcdefghijklmnopqrstuvwxyz0123456789";
    // Git-themed ripple chars: merge symbols, commits, branches
    // const rippleChars = "●○◆◇▸▹←→↑↓⊕⊖⋮⋯│├└┌┐";
    const rippleChars = "@#$%^&*()_+-=[]{};:<>?/\\|";

    const cols = Math.max(baseChars.length, rippleChars.length);
    const rows = 2;
    const glyphCell = 32;
    const atlasW = cols * glyphCell;
    const atlasH = rows * glyphCell;

    const glyphCanvas = document.createElement("canvas");
    glyphCanvas.width = atlasW;
    glyphCanvas.height = atlasH;
    const gctx = glyphCanvas.getContext("2d")!;
    gctx.clearRect(0, 0, atlasW, atlasH);
    gctx.textAlign = "center";
    gctx.textBaseline = "middle";
    const fontSize = Math.floor(glyphCell * 0.8);
    gctx.font = `${fontSize}px "JetBrains Mono", "Menlo", "monaco", monospace`;
    gctx.fillStyle = "white";

    // row 0: base chars
    for (let i = 0; i < baseChars.length; i++) {
      const c = baseChars[i];
      const col = i;
      const row = 0;
      const x = col * glyphCell + glyphCell / 2;
      const y = row * glyphCell + glyphCell / 2;
      gctx.fillText(c, x, y);
    }
    // row 1: ripple chars
    for (let i = 0; i < rippleChars.length; i++) {
      const c = rippleChars[i];
      const col = i;
      const row = 1;
      const x = col * glyphCell + glyphCell / 2;
      const y = row * glyphCell + glyphCell / 2;
      gctx.fillText(c, x, y);
    }

    // create GL texture from glyphCanvas
    const glyphTex = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, glyphTex);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      glyphCanvas,
    );
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.uniform1i(glyphTexLoc, 0);
    gl.uniform1f(glyphColsLoc, cols);
    gl.uniform1f(glyphRowsLoc, rows);

    // ripple state
    const clicks = new Float32Array(MAX_RIPPLES * 2);
    const clickTimes = new Float32Array(MAX_RIPPLES);
    for (let i = 0; i < MAX_RIPPLES; i++) clickTimes[i] = -1;
    let rippleIndex = 0;

    function addClick(clientX: number, clientY: number) {
      const dpr = devicePixelRatio || 1;
      const x = clientX * dpr;
      const y = canvas!.height - clientY * dpr;
      clicks[rippleIndex * 2 + 0] = x;
      clicks[rippleIndex * 2 + 1] = y;
      clickTimes[rippleIndex] = performance.now() / 1000;
      rippleIndex = (rippleIndex + 1) % MAX_RIPPLES;
    }

    function onClick(e: MouseEvent) {
      addClick(e.clientX, e.clientY);
    }
    canvas.addEventListener("click", onClick);

    function resize() {
      if (!canvas || !gl) return;
      const dpr = devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.floor(window.innerWidth * dpr));
      canvas.height = Math.max(1, Math.floor(window.innerHeight * dpr));
      canvas.style.width = "100%";
      canvas.style.height = "100vh";
      gl.viewport(0, 0, canvas.width, canvas.height);
    }

    resize();
    window.addEventListener("resize", resize);

    let raf = 0;
    function render() {
      if (!canvas || !gl) return;
      const time = performance.now() / 1000;

      gl.uniform2f(resLoc, canvas.width, canvas.height);
      gl.uniform1f(timeLoc, time);
      gl.uniform2fv(clickLoc, clicks);
      gl.uniform1fv(clickTimeLoc, clickTimes);

      gl.drawArrays(gl.TRIANGLES, 0, 6);
      raf = requestAnimationFrame(render);
    }
    render();

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("click", onClick);
      try {
        gl.deleteTexture(glyphTex);
        gl.deleteBuffer(buffer);
        gl.deleteProgram(program);
        gl.deleteShader(vs);
        gl.deleteShader(fs);
      } catch (e) {
        // ignore
      }
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: "100%",
        height: "100vh",
        zIndex: 0,
        pointerEvents: "auto",
      }}
    />
  );
};

export default BackgroundShader;
