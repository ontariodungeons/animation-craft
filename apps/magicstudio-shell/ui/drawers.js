// One shell for chat, artwork and the read-only lyrics reader. Contents never remount on toggle.
function createTopDrawers({root=document,canOpen=()=>true,onChange=()=>{},onLayout=()=>{},onSettings=()=>{}}={}) {
  const host=root.getElementById('magic-workstation');let active=null,lastHeight=-1;
  const panels=()=>[...root.querySelectorAll('[data-drawer]')];
  const buttons=()=>[...root.querySelectorAll('[data-drawer-toggle]')];
  const layout=()=>{
    const height=active?host.getBoundingClientRect().height:0;
    if(height!==lastHeight){lastHeight=height;onLayout(height);}
  };
  function show(name,restoreFocus=false) {
    const panel=panels().find(node=>node.dataset.drawer===name);
    if(name&&(!panel||!canOpen()))return false;
    if(name===active)return true;
    const previous=active;active=name;
    for(const node of panels()){node.hidden=node!==panel;node.setAttribute('aria-hidden',String(node!==panel));}
    for(const button of buttons()){
      const open=button.dataset.drawerToggle===name;
      button.setAttribute('aria-expanded',String(open));button.setAttribute('aria-pressed',String(open));
    }
    host.hidden=!name;onChange(name,previous);layout();
    if(restoreFocus)buttons().find(node=>node.dataset.drawerToggle===previous)?.focus();
    return true;
  }
  const api={get active(){return active;},open:name=>show(name),close:(restoreFocus=true)=>show(null,restoreFocus),toggle:name=>show(name===active?null:name),layout};
  root.addEventListener('click',event=>{
    const toggle=event.target.closest('[data-drawer-toggle]');
    if(toggle){api.toggle(toggle.dataset.drawerToggle);return;}
    const settings=event.target.closest('[data-drawer-settings]');
    if(settings){const node=root.getElementById(settings.dataset.drawerSettings);if(node){node.hidden=!node.hidden;settings.setAttribute('aria-expanded',String(!node.hidden));if(!node.hidden)onSettings(node.id);}return;}
    if(event.target.closest('[data-drawer-close]')){api.close();return;}
    if(active&&!host.contains(event.target))api.close(false);
  });
  root.addEventListener('keydown',event=>{
    if(event.key==='Escape'&&active&&!root.getElementById('point-overlay')?.classList.contains('on')){event.preventDefault();api.close();}
  });
  const observer=new ResizeObserver(layout);observer.observe(host);
  window.addEventListener('resize',layout);layout();return api;
}
