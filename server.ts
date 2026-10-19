#!/usr/bin/env bun
/**
 * Image-to-Animation Studio Web App Server (Bun + TypeScript + Python/Blender 5.3/FilmCraft API)
 * Serves the 3D/2D Studio UI at http://127.0.0.1:8765/ and provides live REST API endpoints:
 *  - GET  /api/health
 *  - GET  /api/vault-characters    (Lists the user's real character images in /home/jesse/main-vault/Creative/Design/Characters/)
 *  - GET  /vault-image             (Streams real character images from /home/jesse/main-vault/Creative/Design/Characters/)
 *  - POST /api/intake              (3-Question AI Intake + Functional Topology Decisions)
 *  - POST /api/omarchy-intake      (Triggered by the "🤖 Ask Omarchy Agent" button in the Web App)
 *  - POST /api/run-pipeline        (Executes Track B: Studio -> Live Blender 5.3 -> FilmCraft .fcproj)
 */
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import { join, extname } from 'path';
import {
  buildIntakeQuestions,
  evaluateThreeQuestionIntake,
  buildArchetypeSkeleton,
} from './src/index';

const PORT = Number(process.env.PORT || 8765);
const WORKDIR = import.meta.dir;
const VAULT_ROOT = '/home/jesse/main-vault/Creative/Design/Characters';
const POSES_DIR = join(VAULT_ROOT, '3d poses');

export const VAULT_CHARACTERS: Record<string, {
  id: string;
  name: string;
  cutout: string;
  turnaround: string;
  poseSheet: string;
}> = {
  hopper: {
    id: 'hopper',
    name: 'Hopper (Cream Rabbit Student)',
    cutout: join(VAULT_ROOT, 'Hopper-no-background.png_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_5.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_15.jpeg'),
  },
  whiskers: {
    id: 'whiskers',
    name: 'Whiskers (Grey Tabby Kitten)',
    cutout: join(VAULT_ROOT, 'Whiskers-no-back.png_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_17.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_20.jpeg'),
  },
  scout: {
    id: 'scout',
    name: 'Scout (Explorer Puppy)',
    cutout: join(VAULT_ROOT, 'Scout-no-back.png_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_5.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_5.jpeg'),
  },
  jenny: {
    id: 'jenny',
    name: 'Jenny / Penny (Yellow Chick)',
    cutout: join(VAULT_ROOT, 'Penny.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_9.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_3.jpeg'),
  },
  puddles: {
    id: 'puddles',
    name: 'Puddles (Yellow Duckling)',
    cutout: join(VAULT_ROOT, 'Puddles-tranparent.png_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_12.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_13.jpeg'),
  },
  mr_puddles: {
    id: 'mr_puddles',
    name: 'Mr Puddles (Adult Duck)',
    cutout: join(VAULT_ROOT, 'Mr Puddles.png'),
    turnaround: join(VAULT_ROOT, 'Mr Puddles.png'),
    poseSheet: join(VAULT_ROOT, 'Mr Puddles.png'),
  },
  rusty: {
    id: 'rusty',
    name: 'Rusty (Brown Foal)',
    cutout: join(VAULT_ROOT, 'Rusty-transparent.png_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_19.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_24.jpeg'),
  },
  sam: {
    id: 'sam',
    name: 'Sam (Pink Piglet)',
    cutout: join(VAULT_ROOT, 'Sam.png_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_21.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_33.jpeg'),
  },
  maisy: {
    id: 'maisy',
    name: 'Maisy (Black & White Calf)',
    cutout: join(VAULT_ROOT, 'Slide8.PNG_202607180130.jpeg'),
    turnaround: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_29.jpeg'),
    poseSheet: join(POSES_DIR, 'Character_turnaround_sheet_anima…_202607180130_29.jpeg'),
  },
  old_mac: {
    id: 'old_mac',
    name: 'Old Mac (Farmer Teacher)',
    cutout: join(VAULT_ROOT, 'old_mac-tranparent.png_202607180130.jpeg'),
    turnaround: join(VAULT_ROOT, 'old_mac-tranparent.png_202607180130.jpeg'),
    poseSheet: join(POSES_DIR, 'Create_pose_sheet_for_puppet_202607180130_25.jpeg'),
  },
  miss_hayley: {
    id: 'miss_hayley',
    name: 'Miss Hayley (Teacher)',
    cutout: join(VAULT_ROOT, 'Miss Hayley.png'),
    turnaround: join(VAULT_ROOT, 'Miss Hayley.png'),
    poseSheet: join(VAULT_ROOT, 'Hayley-Draft.jpeg'),
  },
};

const MIME_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.mjs': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.mp4': 'video/mp4',
  '.glb': 'model/gltf-binary',
};

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type',
};

export function createStudioServer(port = PORT) {
  return Bun.serve({
    port,
    async fetch(req) {
      const url = new URL(req.url);

      if (req.method === 'OPTIONS') {
        return new Response(null, { status: 204, headers: corsHeaders });
      }

      if (url.pathname === '/api/health') {
        return Response.json(
          {
            status: 'ok',
            app: 'Image-to-Animation Studio Web App',
            vaultRoot: VAULT_ROOT,
            characters: Object.keys(VAULT_CHARACTERS),
          },
          { headers: corsHeaders }
        );
      }

      if (url.pathname === '/api/vault-characters') {
        return Response.json(VAULT_CHARACTERS, { headers: corsHeaders });
      }

      if (url.pathname === '/vault-image') {
        const charKey = (url.searchParams.get('char') || 'hopper').toLowerCase();
        const imgType = (url.searchParams.get('type') || 'turnaround') as 'turnaround' | 'poseSheet' | 'cutout';
        const directPath = url.searchParams.get('path');
        let targetFile = directPath || '';
        if (!targetFile) {
          const entry = VAULT_CHARACTERS[charKey === 'bunny' ? 'hopper' : charKey] || VAULT_CHARACTERS.hopper;
          targetFile = entry[imgType] || entry.turnaround;
        }
        if (targetFile && existsSync(targetFile) && statSync(targetFile).isFile()) {
          const ext = extname(targetFile).toLowerCase();
          return new Response(readFileSync(targetFile), {
            headers: {
              ...corsHeaders,
              'Content-Type': MIME_TYPES[ext] || 'image/jpeg',
              'Cache-Control': 'public, max-age=3600',
            },
          });
        }
        return new Response('Vault image not found', { status: 404, headers: corsHeaders });
      }

      if ((url.pathname === '/api/intake' || url.pathname === '/api/omarchy-intake') && req.method === 'POST') {
        const body = (await req.json().catch(() => ({}))) as Record<string, any>;
        const subjectHint = body.subjectHint || body.image || 'hopper';
        const q1 = body.q1_motionIntent || body.q1 || 'walk_jump_talk';
        const q2Parts: string[] = Array.isArray(body.q2_sliceParts)
          ? body.q2_sliceParts
          : String(body.q2 || 'head,ears,arms_paws,legs_knees,removable_outfit')
              .split(',')
              .map((s) => s.trim())
              .filter(Boolean);
        const q3 = body.q3_exportTarget || body.q3 || 'filmcraft_mp4';

        const plan = evaluateThreeQuestionIntake({
          q1_motionIntent: q1,
          q2_sliceParts: q2Parts,
          q3_exportTarget: q3,
          subjectHint,
        });
        const intake = buildIntakeQuestions(subjectHint);
        const skeleton = buildArchetypeSkeleton(plan.archetype);

        const slug = subjectHint.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '') || 'hopper';
        const agentCommand = `./studio_pipeline_cli.py run --name "${slug}" --q1 "${q1}" --q2 "${q2Parts.join(',')}" --q3 "${q3}" --verb "${plan.defaultVerb}" --frames 24`;

        return Response.json(
          {
            status: 'ok',
            plan,
            intake,
            skeleton,
            agentCommand,
            skillPath: '/home/jesse/Work/omhas-animation/.agents/skills/image-to-animation/SKILL.md',
          },
          { headers: corsHeaders }
        );
      }

      if (url.pathname === '/api/validate-image' && req.method === 'POST') {
        const body = (await req.json().catch(() => ({}))) as Record<string, any>;
        let targetPath = String(body.imagePath || '');
        if (!targetPath && body.dataUrl) {
          const uploadDir = join(WORKDIR, 'exports', 'custom_uploads');
          mkdirSync(uploadDir, { recursive: true });
          const safeName = String(body.filename || 'uploaded_image.png').replace(/[^a-zA-Z0-9._-]/g, '_');
          targetPath = join(uploadDir, `${Date.now()}_${safeName}`);
          const base64Data = String(body.dataUrl).replace(/^data:image\/[a-zA-Z0-9+.-]+;base64,/, '');
          writeFileSync(targetPath, Buffer.from(base64Data, 'base64'));
        }
        if (!targetPath || !existsSync(targetPath)) {
          return Response.json({ status: 'error', error: 'Image path not found' }, { status: 400, headers: corsHeaders });
        }
        const proc = Bun.spawn(
          ['python3', join(WORKDIR, 'studio_pipeline_cli.py'), 'validate', targetPath],
          { cwd: WORKDIR, stdout: 'pipe', stderr: 'pipe' }
        );
        const stdout = await new Response(proc.stdout).text();
        await proc.exited;
        try {
          return Response.json({ status: 'ok', imagePath: targetPath, report: JSON.parse(stdout) }, { headers: corsHeaders });
        } catch {
          return Response.json({ status: 'error', stdout }, { status: 500, headers: corsHeaders });
        }
      }

      if (url.pathname === '/api/cut-sheet' && req.method === 'POST') {
        const body = (await req.json().catch(() => ({}))) as Record<string, any>;
        let targetPath = String(body.imagePath || '');
        const kind = String(body.kind || 'auto');
        if (!targetPath && body.dataUrl) {
          const uploadDir = join(WORKDIR, 'exports', 'custom_uploads');
          mkdirSync(uploadDir, { recursive: true });
          const safeName = String(body.filename || 'uploaded_sheet.png').replace(/[^a-zA-Z0-9._-]/g, '_');
          targetPath = join(uploadDir, `${Date.now()}_${safeName}`);
          const base64Data = String(body.dataUrl).replace(/^data:image\/[a-zA-Z0-9+.-]+;base64,/, '');
          writeFileSync(targetPath, Buffer.from(base64Data, 'base64'));
        }
        if (!targetPath || !existsSync(targetPath)) {
          return Response.json({ status: 'error', error: 'Image path not found' }, { status: 400, headers: corsHeaders });
        }
        const slug = String(body.slug || body.filename || 'custom_cut')
          .replace(/\.[^.]+$/, '')
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, '_')
          .replace(/^_|_$/g, '') || 'custom_cut';
        const outRel = `exports/pose_and_parts/${slug}`;
        const proc = Bun.spawn(
          [
            'python3',
            join(WORKDIR, 'studio_pipeline_cli.py'),
            'cut-sheet',
            targetPath,
            '--kind',
            kind,
            '--out-dir',
            outRel,
            '--no-ocr',
          ],
          { cwd: WORKDIR, stdout: 'pipe', stderr: 'pipe' }
        );
        const stdout = await new Response(proc.stdout).text();
        const stderr = await new Response(proc.stderr).text();
        await proc.exited;
        try {
          const summary = JSON.parse(stdout);
          const manifestPath = join(WORKDIR, outRel, 'manifest.json');
          const manifest = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf-8')) : null;
          const frameUrls = (manifest?.frames || []).map((f: any) => `/${outRel}/${f.file}`);
          return Response.json(
            {
              status: 'ok',
              summary,
              outDir: `/${outRel}`,
              manifestUrl: `/${outRel}/manifest.json`,
              contactSheetUrl: `/${outRel}/contact_sheet.jpg`,
              frameUrls,
              manifest,
            },
            { headers: corsHeaders }
          );
        } catch {
          return Response.json({ status: 'error', stdout, stderr }, { status: 500, headers: corsHeaders });
        }
      }

      if (url.pathname === '/api/optimize-accuracy') {
        const body = req.method === 'POST' ? ((await req.json().catch(() => ({}))) as Record<string, any>) : {};
        const charSlug = String(body.char || url.searchParams.get('char') || 'hopper').toLowerCase();
        const turnaroundDir = String(body.turnaroundDir || `exports/pose_and_parts/${charSlug}_turnaround`);
        const posesDir = String(body.posesDir || `exports/pose_and_parts/${charSlug}_poses`);
        const outRel = String(body.outDir || `exports/pose_and_parts/${charSlug}`);
        const proc = Bun.spawn(
          [
            'python3',
            join(WORKDIR, 'studio_pipeline_cli.py'),
            'optimize-accuracy',
            '--turnaround-dir',
            turnaroundDir,
            '--poses-dir',
            posesDir,
            '--out-dir',
            outRel,
          ],
          { cwd: WORKDIR, stdout: 'pipe', stderr: 'pipe' }
        );
        const stdout = await new Response(proc.stdout).text();
        const stderr = await new Response(proc.stderr).text();
        await proc.exited;
        try {
          const report = JSON.parse(stdout);
          const regPath = join(WORKDIR, outRel, 'turnaround_registration.json');
          const motionPath = join(WORKDIR, outRel, 'motion_range.json');
          const registration = existsSync(regPath) ? JSON.parse(readFileSync(regPath, 'utf-8')) : null;
          const motionRange = existsSync(motionPath) ? JSON.parse(readFileSync(motionPath, 'utf-8')) : null;
          const registeredFrameUrls = (registration?.frames || []).map(
            (f: any) => `/${outRel}/registered_turnaround/${f.file}`
          );
          return Response.json(
            {
              status: 'ok',
              report,
              registeredFrameUrls,
              registration,
              motionRange,
            },
            { headers: corsHeaders }
          );
        } catch {
          return Response.json({ status: 'error', stdout, stderr }, { status: 500, headers: corsHeaders });
        }
      }

      if (url.pathname === '/api/run-pipeline' && req.method === 'POST') {
        const body = (await req.json().catch(() => ({}))) as Record<string, any>;
        const name = String(body.name || 'hopper').replace(/[^a-zA-Z0-9_-]/g, '_');
        const q1 = String(body.q1 || 'walk_jump_talk');
        const q2 = String(body.q2 || 'head,ears,arms_paws,legs_knees');
        const q3 = String(body.q3 || 'filmcraft_mp4');
        const verb = String(body.verb || 'move_traverse');

        const proc = Bun.spawn(
          [
            'python3',
            join(WORKDIR, 'studio_pipeline_cli.py'),
            'run',
            '--name',
            name,
            '--q1',
            q1,
            '--q2',
            q2,
            '--q3',
            q3,
            '--verb',
            verb,
            '--frames',
            '24',
          ],
          { cwd: WORKDIR, stdout: 'pipe', stderr: 'pipe' }
        );

        const stdout = await new Response(proc.stdout).text();
        const stderr = await new Response(proc.stderr).text();
        await proc.exited;

        try {
          const parsed = JSON.parse(stdout);
          return Response.json({ status: 'ok', pipeline: parsed }, { headers: corsHeaders });
        } catch {
          return Response.json(
            { status: 'error', stdout, stderr, exitCode: proc.exitCode },
            { status: 500, headers: corsHeaders }
          );
        }
      }

      let reqPath = decodeURIComponent(url.pathname);
      if (reqPath === '/' || reqPath === '/index.html') {
        reqPath = '/public/index.html';
      } else if (reqPath === '/studio-bundle.js') {
        reqPath = '/public/studio-bundle.js';
      }

      const filePath = join(WORKDIR, reqPath);
      if (existsSync(filePath) && statSync(filePath).isFile()) {
        const ext = extname(filePath).toLowerCase();
        return new Response(readFileSync(filePath), {
          headers: {
            ...corsHeaders,
            'Content-Type': MIME_TYPES[ext] || 'application/octet-stream',
          },
        });
      }

      return new Response('Not Found', { status: 404, headers: corsHeaders });
    },
  });
}

if (import.meta.main) {
  const srv = createStudioServer(PORT);
  console.log(`🚀 Image-to-Animation Web App running at http://127.0.0.1:${srv.port}/`);
}

