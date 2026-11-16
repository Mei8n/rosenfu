import {expect,test} from 'vitest';
import {readFileSync} from 'node:fs';
const styles=readFileSync(new URL('./style.css',import.meta.url),'utf8');
test('built, plan, candidate, selected and invalid states remain visually distinct at zoom',()=>{
 expect(styles).toContain('.workbench .rail-casing');
 expect(styles).toMatch(/\.workbench \.alignment-path \{[^}]*vector-effect:non-scaling-stroke/);
 expect(styles).toMatch(/\.workbench \.draft-path \{[^}]*stroke-dasharray:7 3/);
 expect(styles).toMatch(/\.workbench \.draft-path\.invalid-candidate \{[^}]*stroke:#c43d45/);
 expect(styles).toMatch(/\.workbench \.draft-path\.track-explicit-selection \{[^}]*stroke:#c17c0b/);
});
