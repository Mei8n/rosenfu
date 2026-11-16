import unittest
import numpy as np
from shapely.geometry import box
from build_coarse import rasterize,encode_world,catalog_for_manifest
class CoarseTests(unittest.TestCase):
    def test_half_cell_threshold_and_narrow_water_limit(self):
        half=rasterize([box(0,0,1/120,1/60)],[0,0,1,1])
        narrow=rasterize([box(0,0,1/240,1/60)],[0,0,1,1])
        self.assertTrue(half[0,0]);self.assertEqual(int(half.sum()),1)
        self.assertFalse(narrow[0,0]);self.assertEqual(int(narrow.sum()),0)
    def test_overlapping_split_polygons_are_not_counted_twice(self):
        g=box(0,0,1/240,1/60)
        self.assertEqual(int(rasterize([g,g],[0,0,1,1]).sum()),0)
    def test_island_hole_remains_model_land(self):
        sea=box(0,0,1,1).difference(box(0,0,1/60,1/60))
        wet=rasterize([sea],[0,0,1,1])
        self.assertFalse(wet[0,0]);self.assertTrue(wet[1,0]);self.assertEqual(int(wet.sum()),3599)
    def test_world_container_row_offsets_and_scope_are_not_interchangeable(self):
        import struct
        wet=np.zeros((60,60),dtype=bool);wet[0,0]=True
        data=encode_world(wet,[122,20,123,21])
        self.assertEqual(data[:4],b'TWW1');self.assertEqual(struct.unpack('<III',data[4:16]),(21600,10800,60))
        offset=16
        def integer():
            nonlocal offset
            n=0;shift=0
            while True:
                b=data[offset];offset+=1;n+=(b&127)<<shift
                if b<128:return n
                shift+=7
        active=[]
        for row in range(10800):
            count=integer();edges=[];end=0
            for _ in range(count):end+=integer();edges.append(end)
            if count:active.append((row,edges))
        self.assertEqual(active,[(6600,[18120,18121])]);self.assertEqual(offset,len(data))
        with self.assertRaises(ValueError):rasterize([],[-180,-85,180,85])
    def test_catalog_cannot_expand_generated_scope(self):
        m=dict(format='rosenfu-scoped-coarse-water',version=1,outside_scope='unknown',scope=[[122,20,154,46]],revision='a'*64,payload=dict(file='japan-coarse.water',bytes=100,sha256='b'*64))
        c=catalog_for_manifest(m);self.assertEqual(c['scope'],m['scope']);self.assertEqual(c['fine'],[])
        c['scope'][0][0]=0;self.assertEqual(m['scope'][0][0],122)
        self.assertEqual(catalog_for_manifest(m,[[130,30,140,40]])['scope'],[[130,30,140,40]])
        for scope in [[[-180,-85,180,85]],[[121,20,154,46]],[[122,20,154,47]]]:
            with self.assertRaises(ValueError):catalog_for_manifest(m,scope)
if __name__=='__main__':unittest.main()
