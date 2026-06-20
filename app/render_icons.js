// Rasterize the two SVGs to 96x96 alpha masks (raw u8 bytes) for embedding.
const fs = require('fs');
const path = require('path');
const { Resvg } = require('@resvg/resvg-js');

const SIZE = 96;
const dir = path.join(__dirname, 'icons');

function rasterize(svg, file, outName) {
  let s = fs.readFileSync(path.join(dir, file), 'utf8');
  // Lucide uses currentColor; make strokes solid black so resvg renders them.
  s = s.replace(/currentColor/g, '#000000');
  const r = new Resvg(s, { fitTo: { mode: 'width', value: SIZE } });
  const img = r.render();
  const px = img.pixels; // RGBA, width*height*4
  const w = img.width, h = img.height;
  // center into a SIZE x SIZE alpha buffer
  const alpha = Buffer.alloc(SIZE * SIZE, 0);
  const ox = Math.floor((SIZE - w) / 2);
  const oy = Math.floor((SIZE - h) / 2);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const a = px[(y * w + x) * 4 + 3];
      const tx = x + ox, ty = y + oy;
      if (tx >= 0 && tx < SIZE && ty >= 0 && ty < SIZE) alpha[ty * SIZE + tx] = a;
    }
  }
  fs.writeFileSync(path.join(dir, outName), alpha);
  console.log(`${outName}: rendered ${w}x${h} -> ${SIZE}x${SIZE}, ${alpha.length} bytes`);
}

rasterize(null, 'github.svg', 'github_alpha.bin');
rasterize(null, 'globe.svg', 'globe_alpha.bin');
