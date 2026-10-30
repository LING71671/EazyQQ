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
  const direct = run(['api', `repos/${repository}/releases/tags/${encodeURIComponent(tag)}`]);
  let release;
  if (direct.status === 0) {
    release = JSON.parse(direct.stdout);
  } else {
    if (!/\(HTTP 404\)/.test(direct.stderr || '')) {
      throw new Error(`Cannot inspect release; publication stopped: ${direct.stderr || 'GitHub CLI failed'}`);
    }
    // GitHub does not index draft releases under the tag endpoint, so an
    // interrupted upload is invisible there; enumerate releases before
    // treating the tag as absent.
    const listing = run(['api', `repos/${repository}/releases?per_page=100`]);
    if (listing.status !== 0) {
      throw new Error(`Cannot inspect releases; publication stopped: ${listing.stderr || 'GitHub CLI failed'}`);
    }
    release = JSON.parse(listing.stdout).find(item => item.tag_name === tag);
    if (!release) return null;
  }
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

// The listing endpoint is cached at the edge, so a release created moments ago
// can still be missing from it. Retry the read before declaring a draft gone.
function sleep(milliseconds) {
  if (milliseconds > 0) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

export function findDraft(options, { attempts = 12, delayMs = 5000 } = {}) {
  for (let attempt = 1; attempt <= attempts; attempt++) {
    const release = inspectRelease(options);
    if (release) return release;
    if (attempt < attempts) sleep(delayMs);
  }
  return null;
}

export function publishRelease({ repository, tag, installer, otherAssets, notes, run = runGh, retry = {} }) {
  const options = { repository, tag, run };
  const attempts = retry.attempts ?? 12;
  const delayMs = retry.delayMs ?? 5000;
  const waitForDraft = () => findDraft(options, { attempts, delayMs });
  if (!findDraft(options, { attempts: Math.min(3, attempts), delayMs })) {
    try {
      mutate(run, ['release', 'create', tag, '--repo', repository, '--verify-tag', '--draft', '--title', `EazyQQ ${tag}`, '--notes-file', notes]);
    } catch (error) {
      // A cached listing can hide an existing draft; only a real conflict on a
      // release that subsequently appears may continue.
      if (!/already exists/i.test(error.message)) throw error;
    }
    if (!waitForDraft()) throw new Error('Release draft disappeared; publication stopped');
  }
  const requireDraft = () => {
    if (!waitForDraft()) throw new Error('Release draft disappeared; publication stopped');
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
