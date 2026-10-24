import test from 'node:test';
import assert from 'node:assert/strict';
import { parseTask, validateTask } from '../frontend/learning-markdown.mjs';
const task='# Import timing\n\n## Input\n$PROJECT, $JSON, $SRT\n\n## Command\n```sh\nmc "$PROJECT" workshop.import_transcript path="$JSON" captions="$SRT"\n```\n\n## Output\nThe existing timing is imported.\n\n## Check\nRead the import receipt; supplied words/times are retained.\n';
test('one bounded task exposes exact action, input and observable output',()=>{
 const b=parseTask(task);assert.equal(b.title,'Import timing');assert.equal(b.action.kind,'command');assert.match(b.action.ref,/workshop.import_transcript/);assert.equal(b.input,'$PROJECT, $JSON, $SRT');assert.ok(b.check);
 assert.deepEqual(validateTask(b,{max_words:100,max_steps:8}),[]);
});
test('missing actionable reference and output are validation failures',()=>{
 const b=parseTask('# Align nicely\n\nRead all these guides.');assert.ok(validateTask(b,{max_words:100,max_steps:8}).some(x=>x.includes('Command, Skill or Source')));
});
test('configured cap rejects rather than truncates existing text',()=>{
 const b=parseTask(task);const before=b.input;b.input+=' '+Array(101).fill('word').join(' ');assert.ok(validateTask(b,{max_words:100,max_steps:8}).some(x=>x.includes('word limit')));assert.ok(b.input.startsWith(before));assert.ok(b.input.includes('word word'));
});
test('command and URL references do not consume human prose cap',()=>{
 const b=parseTask(task);b.action.ref='mc '+('x'.repeat(1200));assert.deepEqual(validateTask(b,{max_words:100,max_steps:8}),[]);
});
