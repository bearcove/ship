const HOME_PREFIX = "/Users/amos/";

function shortenAbsolutePath(path: string): string {
  const worktreeMatch = path.match(/\/\.ship\/worktrees\/[^/]+\//u);
  if (worktreeMatch) {
    return path.slice(worktreeMatch.index! + worktreeMatch[0].length);
  }
  if (path.startsWith(HOME_PREFIX)) {
    return `~/${path.slice(HOME_PREFIX.length)}`;
  }
  return path;
}

const ABSOLUTE_PATH_PATTERN = /\/[A-Za-z0-9._-][^\s"'`)]*/g;

export function formatDisplayPath(path: string): string {
  return shortenAbsolutePath(path);
}

export function formatDisplayText(text: string): string {
  return text.replaceAll(ABSOLUTE_PATH_PATTERN, (value) => shortenAbsolutePath(value));
}
