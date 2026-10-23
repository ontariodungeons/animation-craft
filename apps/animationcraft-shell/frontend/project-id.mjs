// Theatre limits project IDs to 32 characters. Canonical paths still distinguish projects.
export async function theatreProjectId(path){
 const digest=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(path));
 return 'Magic Studio '+Array.from(new Uint8Array(digest).slice(0,8),n=>n.toString(16).padStart(2,'0')).join('');
}
