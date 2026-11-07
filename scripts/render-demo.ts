import { copyFile, mkdir, rename } from 'node:fs/promises';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '..');
const project = resolve(root, 'media/launch/composition');
async function run(args: string[], cwd = project) {
  const process = Bun.spawn(args, { cwd, stdout: 'inherit', stderr: 'inherit' });
  if (await process.exited !== 0) throw new Error(`Demo command failed: ${args[0]} ${args[1]}`);
}
await run(['bun', 'run', 'check']);
await run(['bun', 'run', 'render']);
const film = resolve(project, '../brag.mp4');
const poster = resolve(project, '../brag.jpg');
const baked = resolve(project, '../brag.poster.mp4');
await run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-ss', '18.2', '-i', film, '-frames:v', '1', '-q:v', '2', poster]);
await run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-i', film, '-i', poster, '-filter_complex', "[0:v][1:v]overlay=0:0:enable='eq(n,0)'[v]", '-map', '[v]', '-map', '0:a?', '-c:v', 'libx264', '-crf', '18', '-preset', 'slow', '-pix_fmt', 'yuv420p', '-c:a', 'copy', '-movflags', '+faststart', baked]);
await rename(baked, film);
await mkdir(resolve(root, 'docs/media'), { recursive: true });
await copyFile(film, resolve(root, 'docs/media/earcontrol-demo.mp4'));
await copyFile(poster, resolve(root, 'docs/media/demo-poster.jpg'));
console.log('Demo and poster written to docs/media/.');
