import {readFileSync} from 'node:fs';
import {expect,test} from 'vitest';
test('candidate path is painted above service bands but below station markers',()=>{
 const source=readFileSync(new URL('./App.tsx',import.meta.url),'utf8');
 const services=source.indexOf('<RouteLayer ');
 const candidate=source.indexOf('className="route-candidate-selection"');
 const stations=source.indexOf('<StationLayer ');
 expect(readFileSync(new URL('./RouteLayer.tsx',import.meta.url),'utf8')).toContain('className="service-map-section"');
 expect(readFileSync(new URL('./StationLayer.tsx',import.meta.url),'utf8')).toContain('className={"station-marker"');
 expect(services).toBeGreaterThan(0);
 expect(candidate).toBeGreaterThan(services);
 expect(stations).toBeGreaterThan(candidate);
 const layer=source.slice(candidate,source.indexOf('\n',candidate));
 expect(layer).toContain('pointerEvents="none"');
});
