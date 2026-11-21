// Network-first for the app shell: new deploys show up on the next open; cache is the offline fallback.
const C='pos-v14',API=['/jobs','/tasks','/checkins','/auth','/health','/fun','/study','/comm','/assistant','/finance','/settings','/push'];
self.addEventListener('install',e=>{e.waitUntil(caches.open(C).then(c=>c.addAll(['/','/manifest.webmanifest','/icon-192.png'])).then(()=>self.skipWaiting()))});
self.addEventListener('activate',e=>{e.waitUntil(caches.keys().then(k=>Promise.all(k.filter(x=>x!==C).map(x=>caches.delete(x)))).then(()=>clients.claim()))});
self.addEventListener('fetch',e=>{
  const u=new URL(e.request.url);
  if(e.request.method!=='GET'||u.origin!==location.origin||API.some(p=>u.pathname.startsWith(p)))return;
  e.respondWith(fetch(e.request).then(r=>{const k=r.clone();caches.open(C).then(c=>c.put(e.request,k));return r}).catch(()=>caches.match(e.request)))});
self.addEventListener('push',e=>{let d={};try{d=e.data.json()}catch(_){try{d={body:e.data.text()}}catch(__){d={}}}
 e.waitUntil(self.registration.showNotification(d.title||'Personal OS',{body:d.body||'',icon:'/icon-192.png',badge:'/icon-192.png',data:{url:d.url||'/'},tag:d.tag}))});
self.addEventListener('notificationclick',e=>{e.notification.close();const u=(e.notification.data&&e.notification.data.url)||'/';
 e.waitUntil(clients.matchAll({type:'window',includeUncontrolled:true}).then(ws=>{for(const w of ws){if('focus'in w){if('navigate'in w){w.navigate(u).catch(()=>{})}return w.focus()}}return clients.openWindow(u)}))});
