import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const tokenFiles = ['src/styles/tokens.css', 'viewer/src/styles/tokens.css'];
const requiredTokens = [
  '--canvas', '--surface', '--group', '--control', '--text', '--muted',
  '--accent', '--success', '--danger', '--focus-ring', '--motion-fast',
];

for (const file of tokenFiles) {
  const css = await readFile(file, 'utf8');
  for (const token of requiredTokens) {
    assert(css.includes(token), `${file} is missing ${token}`);
  }
}

console.log('design token contract passed');
