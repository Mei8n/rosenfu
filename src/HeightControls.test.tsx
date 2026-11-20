// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {HeightControls} from './HeightControls';
import {HEIGHT_COST_FACTORS} from './heightCostModel';
import {LEVEL_LABELS} from './sectionInfrastructure';
afterEach(cleanup);
test('seven compact ordered choices show current level without fabricated costs',()=>{
 const change=vi.fn();render(<HeightControls value="ground" disabled={false} onChange={change}/>);
 expect(screen.getAllByRole('button').map(button=>button.textContent)).toEqual(['+3','+2','+1','0','−1','−2','−3']);
 expect(screen.getByRole('button',{name:LEVEL_LABELS.ground}).getAttribute('aria-pressed')).toBe('true');
 for(const [level,factors] of Object.entries(HEIGHT_COST_FACTORS)){
  const button=screen.getByRole('button',{name:LEVEL_LABELS[level as keyof typeof LEVEL_LABELS]});fireEvent.mouseEnter(button.parentElement!);
  expect(screen.getByRole('tooltip').textContent).toContain(`建設費 ${factors.construction}倍・1 kmあたり 未実装`);
  expect(screen.getByRole('tooltip').textContent).toContain(`維持費 ${factors.maintenance}倍`);fireEvent.mouseLeave(button.parentElement!);
 }
 fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.underground_3}));expect(change).toHaveBeenCalledWith('underground_3');
});
test('keyboard focus exposes tooltip and disabled construction cannot change height',()=>{
 const change=vi.fn();const {rerender}=render(<HeightControls value="elevated" disabled={false} onChange={change}/>);
 const button=screen.getByRole('button',{name:LEVEL_LABELS.elevated});fireEvent.focus(button);expect(screen.getByRole('tooltip').id).toBe(button.getAttribute('aria-describedby'));fireEvent.blur(button);expect(screen.queryByRole('tooltip')).toBeNull();
 rerender(<HeightControls value="elevated" disabled onChange={change}/>);fireEvent.click(button);expect(change).not.toHaveBeenCalled();
});
