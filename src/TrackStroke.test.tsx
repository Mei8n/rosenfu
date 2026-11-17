// @vitest-environment jsdom
import {cleanup,render} from '@testing-library/react';
import {afterEach,expect,test} from 'vitest';
import {TrackStroke} from './TrackStroke';
afterEach(cleanup);
test('casing and rail use identical geometry without adding hit targets or metadata',()=>{
 const {container}=render(<svg><TrackStroke d="M0 0 L50 30" className="draft-path invalid-candidate" data-plan-id="p"/></svg>);
 const paths=container.querySelectorAll('path');expect(paths).toHaveLength(2);
 expect(paths[0].getAttribute('d')).toBe(paths[1].getAttribute('d'));
 expect(paths[0].getAttribute('aria-hidden')).toBe('true');
 expect(paths[1].getAttribute('data-plan-id')).toBe('p');expect(paths[1].getAttribute('pointer-events')).toBe('none');
 expect(container.querySelectorAll('.draft-path')).toHaveLength(1);expect(container.querySelector('title')).toBeNull();
});

test('separate crossing strokes retain per-track casing occlusion instead of a global paint batch',()=>{
 const {container}=render(<svg><TrackStroke d="M0 25 L50 25" className="alignment-path"/><TrackStroke d="M25 0 L25 50" className="alignment-path"/></svg>);
 const paths=Array.from(container.querySelectorAll('path'));
 expect(paths.map(path=>path.getAttribute('class'))).toEqual(['rail-casing','alignment-path','rail-casing','alignment-path']);
 expect(paths[2].getAttribute('d')).toBe(paths[3].getAttribute('d'));
});
