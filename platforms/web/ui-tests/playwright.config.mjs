import {defineConfig,devices} from '@playwright/test';
export default defineConfig({
  testDir:'.',testMatch:'**/*.spec.mjs',timeout:30000,fullyParallel:false,workers:3,
  use:{baseURL:'http://127.0.0.1:8765',headless:true},
  webServer:{command:'python3 -m http.server 8765 --bind 127.0.0.1 --directory ../../../target/packages/web',url:'http://127.0.0.1:8765',reuseExistingServer:!process.env.CI},
  projects:[{name:'chromium',use:{...devices['Desktop Chrome']}},{name:'webkit',use:{...devices['Desktop Safari']}},{name:'android-webview-profile',use:{...devices['Pixel 7'],browserName:'chromium'}}],
});
