import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const defaultRepository = 'LING71671/EazyQQ';

function runGh(args) {
  const result = spawnSync('gh', args, { encoding: 'utf8', windowsHide: true });
  if (result.error) throw result.error;
  return result;
}

export function inspectRelease({ repository, tag, run = runGh }) {
  const result = run(['api', `repos/${repository}/releases/tags/${encodeURIComponent(tag)}`]);
  if (result.status !== 0) {
    if (/\(HTTP 404\)/.test(result.stderr || '')) return null;
    throw new Error(`Cannot inspect release; publication stopped: ${result.stderr || 'GitHub CLI failed'}`);
  }
  const release = JSON.parse(result.stdout);
  if (release.tag_name !== tag || typeof release.draft !== 'boolean') {
    throw new Error('GitHub returned an unexpected release identity or draft state');
  }
  if (!release.draft) throw new Error(`Release ${tag} is already public; its assets and notes are immutable`);
  return release;
}

function mutate(run, args) {
  const result = run(args);
  if (result.status !== 0) throw new Error(result.stderr || 'GitHub release operation failed');
}

export function publishRelease({ repository, tag, installer, otherAssets, notes, run = runGh }) {
  const options = { repository, tag, run };
  if (!inspectRelease(options)) {
    mutate(run, ['release', 'create', tag, '--repo', repository, '--verify-tag', '--draft', '--title', `EazyQQ ${tag}`, '--notes-file', notes]);
  }
  const requireDraft = () => {
    if (!inspectRelease(options)) throw new Error('Release draft disappeared; publication stopped');
  };
  // Recheck before each write, including resumptions after a partial upload.
  requireDraft();
  mutate(run, ['release', 'upload', tag, '--repo', repository, installer, '--clobber']);
  requireDraft();
  mutate(run, ['release', 'upload', tag, '--repo', repository, ...otherAssets, '--clobber']);
  requireDraft();
  mutate(run, ['release', 'edit', tag, '--repo', repository, '--draft=false', '--prerelease=false', '--latest', '--title', `EazyQQ ${tag}`, '--notes-file', notes]);
}

export function main(args = process.argv.slice(2)) {
  if (args.some(arg => arg !== '--check')) throw new Error('Only --check is supported');
  const root = process.cwd();
  const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
  const tag = process.env.RELEASE_TAG;
  const repository = process.env.GITHUB_REPOSITORY || defaultRepository;
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository) || tag !== `v${version}`) {
    throw new Error('Release repository or tag does not match the source manifest');
  }
  const options = { repository, tag };
  if (args.includes('--check')) {
    inspectRelease(options);
    console.log(`Release ${tag} is absent or draft; public releases cannot be overwritten`);
    return;
  }
  const directory = path.join(root, 'output/release', version);
  const installer = path.join(directory, `EazyQQ_${version}_x64-setup.exe`);
  const otherAssets = ['eazyqq-cli-windows-x64.zip', 'eazyqq_cli.exe', 'ezq.exe', 'SHA256SUMS.txt', 'latest.json'].map(name => path.join(directory, name));
  const notes = path.join(root, `docs/release/${tag}.md`);
  for (const file of [installer, ...otherAssets, notes]) {
    if (!fs.statSync(file).isFile() || !fs.statSync(file).size) throw new Error(`Release file is missing or empty: ${file}`);
  }
  publishRelease({ ...options, installer, otherAssets, notes });
  console.log(`Published ${tag} after validating the draft at every write boundary`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
