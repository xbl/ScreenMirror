import fs from 'node:fs';

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`Invalid release version: ${version ?? '(missing)'}`);
}

const updateJson = (file) => {
  const data = JSON.parse(fs.readFileSync(file, 'utf8'));
  data.version = version;
  if (file.endsWith('package-lock.json')) data.packages[''].version = version;
  fs.writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
};

updateJson('package.json');
updateJson('package-lock.json');
updateJson('viewer/package.json');
updateJson('viewer/package-lock.json');

const replaceOnce = (file, pattern, replacement) => {
  const input = fs.readFileSync(file, 'utf8');
  const output = input.replace(pattern, replacement);
  if (output === input) throw new Error(`Version field not found in ${file}`);
  fs.writeFileSync(file, output);
};

replaceOnce('src-tauri/Cargo.toml', /(^version = ")[^"]+("\n)/m, `$1${version}$2`);
replaceOnce('src-tauri/tauri.conf.json', /("version": ")[^"]+(")/, `$1${version}$2`);
replaceOnce(
  'src-tauri/Cargo.lock',
  /(\[\[package\]\]\nname = "screenmirror"\nversion = ")[^"]+(")/,
  `$1${version}$2`,
);
