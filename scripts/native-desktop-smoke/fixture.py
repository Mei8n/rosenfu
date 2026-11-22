"""Generated, synthetic OpenMapTiles-format fixture. No network access or real roads."""
import base64
import hashlib
import math

ORIGIN = "https://tiles.openfreemap.org"
REVISION = "rosenfu_webview_fixture_16"
METADATA = f"{ORIGIN}/planet"
TILE_TEMPLATE = METADATA + "/" + REVISION + "/{z}/{x}/{y}.pbf"
EXTENT = 4096
CAMERA = (35.67504,139.75,16)
LEFT = (14551,6452)
RIGHT = (14552,6452)
# Both lines overlap the seam with independently matching integer geometry;
# neither anchor is a boundary vertex. Encoded one-way is west -> east, so
# the gameplay test selects the reverse (east -> west).
WEST = [(3900,260),(3970,350),(4200,350)]
EAST = [(-104,350),(1150,350)]
V1 = [(670,300),(1190,390),(1510,390)]
# Three distinct authored points, three directed roads; not a corridor reversed.
LOOP = ((450,300),(950,300),(950,700))
LOOP_LINES = [(LOOP[i],LOOP[(i+1)%3]) for i in range(3)]


def v(value):
    if value < 0:
        raise ValueError("negative encoded protobuf varint")
    out = bytearray()
    while value >= 128:
        out.append((value & 127) | 128)
        value >>= 7
    return bytes(out)+bytes([value])


def n(field,value):
    return v(field*8)+v(value)


def blob(field,data):
    return v(field*8+2)+v(len(data))+data


def z(value):
    return 2*value if value>=0 else -2*value-1


def vector_cell(points, *, oneway=True, road_class="minor"):
    return vector_lines([points],oneway=oneway,road_class=road_class)


def vector_lines(lines, *, oneway=True, road_class="minor"):
    features=[]
    for points in lines:
        x,y=points[0]
        geom=v(9)+v(z(x))+v(z(y))+v((len(points)-1)*8+2)
        for nx,ny in points[1:]:
            geom += v(z(nx-x))+v(z(ny-y))
            x,y=nx,ny
        feature=blob(2,bytes([0,0,1,1] if oneway else [0,0]))+n(3,2)+blob(4,geom)
        features.append(blob(2,feature))
    layer=blob(1,b"transportation")+n(15,2)+n(5,EXTENT)+b''.join(features)
    layer += blob(3,b"class")+blob(4,blob(1,road_class.encode()))
    if oneway:
        layer += blob(3,b"oneway")+blob(4,n(4,1))
    return blob(3,layer)


def point(tile, xy):
    x,y=tile
    xx,yy=xy
    return (math.degrees(math.atan(math.sinh(math.pi*(1-2*(y+yy/EXTENT)/16384)))),
            (x+xx/EXTENT)*360/16384-180)


def projection(at, camera=CAMERA):
    latitude,longitude=at
    lat0,lon0,zoom=camera
    m=lambda lat: (1-math.asinh(math.tan(math.radians(lat)))/math.pi)/2
    scale=256*2**zoom
    return [500+(longitude-lon0)*scale/360,300+(m(latitude)-m(lat0))*scale]


def key(tile):
    return TILE_TEMPLATE.format(z=14,x=tile[0],y=tile[1])


def config(scenario):
    if scenario=="loop":
        tiles={key(RIGHT):vector_lines(LOOP_LINES)}
        anchors=[{"key":key(RIGHT),"feature":f,"vertex":i,"geo":point(RIGHT,xy)}
                 for (f,i),xy in zip(((0,0),(0,1),(1,1)),LOOP)]
    elif scenario in ("seam","single"):
        lines={LEFT:WEST,RIGHT:EAST} if scenario=="seam" else {RIGHT:V1}
        endpoints=[(RIGHT,1),(LEFT,0)] if scenario=="seam" else [(RIGHT,2),(RIGHT,0)]
        tiles={key(tile):vector_cell(line) for tile,line in lines.items()}
        anchors=[{"key":key(tile),"feature":0,"vertex":index,"geo":point(tile,lines[tile][index])} for tile,index in endpoints]
    else:
        raise ValueError(f"unknown test {scenario}")
    # A real, noneligible path fixture avoids pretending that an omitted
    # transportation layer establishes absence of roads in surrounding cells.
    neighbour=vector_cell([(1700,2600),(1900,2600)],oneway=False,road_class="path")
    nearby={key((x,y)):base64.b64encode(neighbour).decode()
             for x in range(14548,14557) for y in range(6448,6457)}
    nearby.update({url:base64.b64encode(raw).decode() for url,raw in tiles.items()})
    return {"meta":METADATA,"template":TILE_TEMPLATE,"responses":nearby,
        "primaryShas":{url:hashlib.sha256(raw).hexdigest() for url,raw in tiles.items()},
        "anchors":anchors}
