import { copyFile, rename, writeFile } from 'node:fs/promises';

const out = new URL('../apps/desktop/dist-pages/', import.meta.url);
const releases = 'https://github.com/j1seki/omniscreenveil/releases';
await rename(new URL('index.html', out), new URL('demo.html', out));
await copyFile(new URL('../docs/images/app-dark-demo.gif', import.meta.url), new URL('walkthrough.gif', out));
await writeFile(new URL('index.html', out), `<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>OmniScreenVeil · selective screen-share privacy</title>
<meta name="description" content="OmniScreenVeil for Windows, macOS, and Linux. Filter selected windows from a dedicated share surface.">
<link rel="icon" href="./veil-mark.svg" type="image/svg+xml">
<style>
:root{color-scheme:dark;background:#0e1220;color:#edf0ff;font:16px/1.55 system-ui,sans-serif}body{margin:0}main{max-width:980px;padding:40px 22px 64px;margin:auto}header{border-bottom:1px solid #29334a;padding-bottom:32px}h1{font-size:clamp(2.5rem,9vw,5rem);line-height:1;letter-spacing:-.06em;margin:.25em 0}p{color:#b7c3d9}a{color:#a5d9ff}nav,.cards{display:flex;flex-wrap:wrap;gap:12px}nav a{border:1px solid #456285;border-radius:10px;padding:9px 15px;text-decoration:none}nav a:first-child{background:#b4dffb;color:#112339}.cards article{flex:1 1 240px;background:#171e30;border:1px solid #293852;border-radius:12px;padding:16px}.cards h3{margin:0}section{margin-top:32px}img.demo{width:100%;height:auto;border-radius:12px}footer{margin-top:30px;color:#b7c3d9}small{color:#b7c3d9}
</style><main>
<header><img src="./veil-mark.svg" width="58" height="58" alt=""><h1>OmniScreenVeil</h1>
<p>Keep selected windows out of your share without rearranging your desktop. Share the dedicated Veil Surface and check what your audience can see.</p>
<nav aria-label="Downloads and demo"><a href="${releases}/latest">Download stable</a><a href="${releases}">See prereleases</a><a href="./demo.html">Try the browser demo</a></nav>
<p><small>The browser demo uses sample windows. Real capture and protection require the desktop app.</small></p></header>
<section><h2>Choose your platform</h2><div class="cards">
<article><h3>Windows</h3><p>x64 installer or portable ZIP. Supports Veil Surface, Strict Hide, and native Exclusion where the capture app respects it. Native streaming uses a selected display.</p><a href="${releases}">Windows downloads</a></article>
<article><h3>macOS</h3><p>Apple Silicon DMG, macOS 14+. ScreenCaptureKit filters the Surface; Accessibility enables Strict Hide and more complete window detection. No system-wide Exclusion.</p><a href="${releases}">macOS downloads</a></article>
<article><h3>Linux</h3><p>x64 AppImage. Use X11, or Sway/Hyprland with grim for Surface capture. Other Wayland compositors don’t expose equivalent supported capture and window controls.</p><a href="${releases}">Linux downloads</a></article>
</div></section>
<section><h2>How it works</h2><p>Choose a protection mode per app. Arm or disarm without losing your rules. Use Capture Test with a harmless window before sharing anything sensitive.</p><details><summary>Watch an earlier desktop walkthrough</summary><img class="demo" loading="lazy" src="./walkthrough.gif" alt="Earlier OmniScreenVeil desktop walkthrough"></details></section>
<footer><a href="https://github.com/j1seki/omniscreenveil">Source, documentation, and issues on GitHub</a></footer></main></html>`);
