precision highp float;

uniform vec2 u_resolution;
uniform float u_time;

const int MAX_RIPPLES = 8;
uniform vec2 u_clicks[MAX_RIPPLES];
uniform float u_clickTimes[MAX_RIPPLES];

uniform sampler2D u_glyphs;
uniform float u_glyphCols;
uniform float u_glyphRows;

// ===== hash functions for randomness =====
float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float hash2(vec2 p) {
  return fract(sin(dot(p, vec2(269.5, 183.3))) * 43758.5453);
}

vec2 hash22(vec2 p) {
  return vec2(hash(p), hash2(p));
}

// fetch glyph sample from atlas (returns alpha)
float sampleGlyph(vec2 uvInCell, float col, float row) {
  vec2 atlasUV = (vec2(col, row) + vec2(uvInCell.x, 1.0 - uvInCell.y)) / vec2(u_glyphCols, u_glyphRows);
  vec4 tex = texture2D(u_glyphs, atlasUV);
  return tex.a;
}

// ======== PROCEDURAL GLYPH ========
float drawGlyph(vec2 uv, float h) {
    uv = uv * 2.0 - 1.0;
    if (h < 0.2) {
      return 1.0 - smoothstep(0.4, 0.5, length(uv));
    } else if (h < 0.4) {
      return step(abs(uv.x), 0.1);
    } else if (h < 0.6) {
      return step(abs(uv.y), 0.1);
    } else if (h < 0.8) {
      return step(abs(uv.x + uv.y), 0.1);
    } else {
      return step(abs(uv.x - uv.y), 0.1);
    }
}

// Firecracker particle burst effect
float fireworkParticle(vec2 pixel, vec2 origin, float age, float particleId, float maxAge) {
    // Each particle has unique direction and speed
    float angle = particleId * 6.2831853 * 0.618033988; // golden ratio for even distribution
    float speed = 150.0 + hash(vec2(particleId, 1.0)) * 200.0;
    float gravity = 80.0;
    
    // Particle trajectory with gravity
    vec2 velocity = vec2(cos(angle), sin(angle)) * speed;
    vec2 particlePos = origin + velocity * age - vec2(0.0, gravity * age * age);
    
    // Spark trail effect
    float dist = distance(pixel, particlePos);
    float fade = 1.0 - smoothstep(0.0, maxAge * 0.8, age);
    float sparkSize = mix(8.0, 2.0, age / maxAge);
    float spark = smoothstep(sparkSize, 0.0, dist) * fade;
    
    // Trail effect
    float trailLen = 20.0 * (1.0 - age / maxAge);
    vec2 trailDir = normalize(velocity - vec2(0.0, gravity * age * 2.0));
    float trailDist = abs(dot(pixel - particlePos, vec2(-trailDir.y, trailDir.x)));
    float alongTrail = dot(pixel - particlePos, -trailDir);
    float trail = smoothstep(3.0, 0.0, trailDist) * smoothstep(trailLen, 0.0, alongTrail) * step(0.0, alongTrail) * fade * 0.5;
    
    return spark + trail;
}

void main() {
  vec2 pixel = gl_FragCoord.xy;
  float cellSize = 12.0;
  vec2 cell = floor(pixel / cellSize);
  vec2 uv = fract(pixel / cellSize);

  float rBase = hash(cell);
  float rRipple = hash(cell * 1.37 + 12.3);
  
  float colBase = floor(rBase * u_glyphCols);
  float colRipple = floor(rRipple * u_glyphCols);
  
  float rowBase = 0.0;
  float rowRipple = 1.0;
  
  float baseMask = sampleGlyph(uv, colBase, rowBase);
  float rippleMask = sampleGlyph(uv, colRipple, rowRipple);
  
  float r = hash(cell);
  float gmask = drawGlyph(uv, r);

  // Clean minimal background
  vec3 base = vec3(1.0);
  vec3 grid = vec3(0.94);

  vec2 m = mod(pixel, cellSize);
  float line = min(step(m.x, 1.0), step(m.y, 1.0));
  vec3 color = mix(base, grid, line);

  float maxAge = 2.0;
  float charInfluence = 0.0;
  float particleGlow = 0.0;

  vec3 primaryColor = vec3(1.0, 0.44, 0.12); 
  vec3 sparkColor = vec3(1.0, 0.7, 0.3);       // Bright spark

  for (int i = 0; i < MAX_RIPPLES; i++) {
    if (u_clickTimes[i] < 0.0) continue;

    float age = u_time - u_clickTimes[i];
    if (age > maxAge) continue;
    
    vec2 origin = u_clicks[i];
    float d = distance(pixel, origin);

    // === INITIAL EXPLOSION FLASH ===
    float flashRadius = 50.0 * smoothstep(0.0, 0.05, age) * (1.0 - smoothstep(0.05, 0.2, age));
    float flash = smoothstep(flashRadius + 20.0, flashRadius, d) * (1.0 - smoothstep(0.0, 0.15, age));
    
    // === FIRECRACKER PARTICLES ===
    float numParticles = 24.0;
    for (float p = 0.0; p < 24.0; p++) {
        particleGlow += fireworkParticle(pixel, origin, age, p + float(i) * 100.0, maxAge) * 0.15;
    }
    
    // === EXPANDING SHOCKWAVE RING ===
    float ringSpeed = 250.0;
    float ringDist = age * ringSpeed;
    float ringWidth = 15.0;
    float blurAmount = 8.0;
    float innerEdge = smoothstep(ringDist - ringWidth - blurAmount, ringDist - ringWidth, d);
    float outerEdge = 1.0 - smoothstep(ringDist, ringDist + blurAmount, d);
    float ring = innerEdge * outerEdge;
    float ringFade = 1.0 - smoothstep(maxAge * 0.4, maxAge * 0.8, age);
    ring *= ringFade;

    // === SCATTER EFFECT ON GLYPHS ===
    float scatterRadius = age * 180.0;
    float scatterFade = 1.0 - smoothstep(maxAge * 0.3, maxAge, age);
    float scatter = smoothstep(scatterRadius + 50.0, scatterRadius - 30.0, d) * scatterFade;
    
    charInfluence = max(charInfluence, ring + scatter * 0.5 + flash);
  }

  // Clamp influences
  particleGlow = clamp(particleGlow, 0.0, 1.0);
  charInfluence = clamp(charInfluence, 0.0, 1.0);

  // Glyph transition
  float useRipple = smoothstep(0.05, 0.35, charInfluence);
  float glyphMask = mix(baseMask, rippleMask, useRipple);

  color -= gmask * 0.03;

  // Subtle overall glow
  color = mix(color, primaryColor * 0.5 + 0.5, charInfluence * 0.1);

  // Apply git-themed coloring
  color = mix(color, primaryColor, glyphMask * charInfluence * 0.9);
  
  // Add spark particles
  color = mix(color, sparkColor, particleGlow);
  

  gl_FragColor = vec4(color, 1.0);
}
