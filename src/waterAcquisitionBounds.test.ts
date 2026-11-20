import {expect,test} from 'vitest';
import {waterAcquisitionBounds} from './waterAcquisitionBounds';
test('acquires local view and active line geometry without using nationwide view extent',()=>{expect(waterAcquisitionBounds([],{longitude:139.75,latitude:35.67})).toEqual([139.7,35.65,139.8,35.7]);const b=waterAcquisitionBounds([{longitude:130,latitude:33}],{longitude:139.75,latitude:35.67});expect(b[0]).toBe(130);expect(b[1]).toBe(33);expect(b[2]).toBeLessThan(140);});
