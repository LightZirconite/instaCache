const { app, BrowserWindow } = require('electron');
const url = process.env.BENCH_URL;
app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');
app.whenReady().then(() => {
  const w = new BrowserWindow({
    width: 1280, height: 900,
    webPreferences: { backgroundThrottling: false },
  });
  w.loadURL(url);
});
