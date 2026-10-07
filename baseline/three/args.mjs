// Shared command-line parsing for the baseline scripts: --key value, or --flag (true).
export function parseArgs(argv = process.argv.slice(2)) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) continue;
    const next = argv[i + 1];
    if (next === undefined || next.startsWith('--')) args[a.slice(2)] = true;
    else args[a.slice(2)] = argv[++i];
  }
  return args;
}

export const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';

// Real GPU in headless Chrome (ANGLE D3D11) and uncapped frames unless --vsync (decision 0020).
export function chromeArgs(args) {
  return [
    '--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--window-size=1920,1080',
    ...(args.vsync ? [] : ['--disable-gpu-vsync', '--disable-frame-rate-limit']),
  ];
}
