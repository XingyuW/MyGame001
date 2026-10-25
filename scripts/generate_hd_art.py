"""Original, reproducible isometric miniatures. Python 3 + Pillow, no downloaded art.
Four texture pixels per world pixel; geometry is re-rasterized, never enlarged.
Ground contracts: buildings (16 + d*32, 88), units (48, 70).

This file holds the `Model` canvas (projection, the quick Pillow preview rasterizer) and the
helpers the terrain tops still use. The buildings and units themselves are modelled from
real-world references, in metres, in hd_buildings.py, hd_vehicles.py, hd_infantry.py,
hd_aircraft.py and hd_naval.py on top of hd_kit.py; `building()` and `unit()` below dispatch to
them. render_hd_blender.py renders the shipped sprites; `Model.render` is only a flat preview.
"""
from pathlib import Path
from math import sin, cos, pi, sqrt
import re, random
import numpy as np
try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:  # Blender's bundled Python imports only the geometry (render_hd_blender.py).
    Image = ImageDraw = ImageFilter = None
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/sprites/hd'
S = 4
BLUE=(30,112,205); RED=(168,43,32); STEEL=(123,144,151); WHITE=(184,197,188)
DARK=(32,43,49); GOLD=(211,163,64); GLASS=(74,167,174); OLIVE=(112,121,75)
ZC = 32*sqrt(2)*cos(pi/6)  # screen px of height per cell of true 3D length (2:1 dimetric)
class C(tuple):
    """An RGB colour that names its material class (hd_kit.py lists them); render_hd_blender.py
    gives each class its own physically based shader. Plain tuples keep the legacy look-up."""
    def __new__(cls,rgb,mat='paint'):
        o=tuple.__new__(cls,rgb); o.mat=mat; return o
def shade(c,f):
    out=tuple(max(0,min(255,int(v*f))) for v in c)
    mat=getattr(c,'mat',None)
    return C(out,mat) if mat else out
class Model:
    def __init__(self,w,h,origin,angle=0):
        self.w,self.h,self.o,self.angle=w,h,origin,angle
        self.faces=[]; self.lines=[]
    def rotate(self,p):
        x,y,z=p
        return (x*cos(self.angle)-y*sin(self.angle),x*sin(self.angle)+y*cos(self.angle),z)
    def project(self,p):
        x,y,z=self.rotate(p)
        return ((self.o[0]+(x-y)*32)*S,(self.o[1]+(x+y)*16-z)*S)
    def face(self,pts,c):
        # Directional warm key / cool ambient light, consistent across every miniature.
        rp=[self.rotate(p) for p in pts]
        a,b=rp[0],rp[1]; d=rp[2]
        u=(b[0]-a[0],b[1]-a[1],(b[2]-a[2])/32)
        v=(d[0]-a[0],d[1]-a[1],(d[2]-a[2])/32)
        n=(u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0])
        length=sqrt(sum(q*q for q in n)) or 1
        light=.68+.42*max(0,(-.35*n[0]+.7*n[1]+.9*n[2])/length)
        depth=sum(x+y+z/32 for x,y,z in rp)/len(rp)
        self.faces.append((depth,pts,shade(c,light)))
    def box(self,x,y,z,w,d,h,c,bevel=True):
        pts=[(x,y,z),(x+w,y,z),(x+w,y+d,z),(x,y+d,z),(x,y,z+h),(x+w,y,z+h),(x+w,y+d,z+h),(x,y+d,z+h)]
        for ids in [(0,4,7,3),(1,2,6,5),(0,1,5,4),(3,7,6,2),(4,5,6,7)]: self.face([pts[i] for i in ids],c)
        if bevel and h>2:
            self.face([(x,y+d,z+h),(x+w,y+d,z+h),(x+w,y+d,z+h-.65),(x,y+d,z+h-.65)],shade(c,1.24))
    def cyl(self,x,y,z,r,h,c,n=24,r2=None):
        r2=r if r2 is None else r2
        for i in range(n):
            a,b=i*2*pi/n,(i+1)*2*pi/n
            self.face([(x+r*cos(a),y+r*sin(a),z),(x+r*cos(b),y+r*sin(b),z),(x+r2*cos(b),y+r2*sin(b),z+h),(x+r2*cos(a),y+r2*sin(a),z+h)],c)
        self.face([(x+r2*cos(i*2*pi/n),y+r2*sin(i*2*pi/n),z+h) for i in range(n)],shade(c,1.1))
    def ellipsoid(self,x,y,z,rx,ry,rz,c):
        for j in range(10):
            a=-pi/2+j*pi/10; b=a+pi/10
            for i in range(24):
                u=i*2*pi/24; v=u+2*pi/24
                self.face([(x+rx*cos(t)*cos(s),y+ry*cos(t)*sin(s),z+rz*sin(t)) for t,s in [(a,u),(a,v),(b,v),(b,u)]],c)
    # Primitive grouping hooks: no-ops here; the Blender backend smooths faces within a group.
    def begin(self,smooth=False): pass
    def end(self): pass
    def tube(self,a,b,r,c,n=12,caps=True):
        # Cylinder between arbitrary points (x/y in cells, z in screen px); r in cells.
        A=(a[0],a[1],a[2]/ZC);B=(b[0],b[1],b[2]/ZC)
        ax=[B[i]-A[i] for i in range(3)];L=sqrt(sum(q*q for q in ax)) or 1;ax=[q/L for q in ax]
        ref=(0,0,1) if abs(ax[2])<.9 else (1,0,0)
        u=[ax[1]*ref[2]-ax[2]*ref[1],ax[2]*ref[0]-ax[0]*ref[2],ax[0]*ref[1]-ax[1]*ref[0]]
        ul=sqrt(sum(q*q for q in u));u=[q/ul for q in u]
        v=[ax[1]*u[2]-ax[2]*u[1],ax[2]*u[0]-ax[0]*u[2],ax[0]*u[1]-ax[1]*u[0]]
        def ringpts(P):
            out=[]
            for i in range(n):
                t=i*2*pi/n;p=[P[k]+r*(cos(t)*u[k]+sin(t)*v[k]) for k in range(3)]
                out.append((p[0],p[1],p[2]*ZC))
            return out
        ra,rb=ringpts(A),ringpts(B)
        self.begin(n>=10)
        for i in range(n):
            j=(i+1)%n;self.face([ra[i],ra[j],rb[j],rb[i]],c)
        if caps:self.face(rb[::-1],shade(c,1.05));self.face(ra,c)
        self.end()
    def prism(self,profile,y0,y1,c,smooth=False):
        # Extrude a convex x/z profile (cells, px) across y: sloped glacis plates, track runs.
        self.begin(smooth)
        n=len(profile)
        for i in range(n):
            (x0,z0),(x1,z1)=profile[i],profile[(i+1)%n]
            self.face([(x0,y0,z0),(x1,y0,z1),(x1,y1,z1),(x0,y1,z0)],c)
        self.face([(x,y0,z) for x,z in profile],c);self.face([(x,y1,z) for x,z in profile[::-1]],c)
        self.end()
    def beam(self,a,b,r,c):
        # Thin geometry rather than screen-space lines: rotates with the model.
        self.face([(a[0],a[1],a[2]-r),(b[0],b[1],b[2]-r),(b[0],b[1],b[2]+r),(a[0],a[1],a[2]+r)],c)
        self.face([(a[0]-.02,a[1],a[2]),(b[0]-.02,b[1],b[2]),(b[0]+.02,b[1],b[2]),(a[0]+.02,a[1],a[2])],shade(c,1.2))
    def render(self):
        im=Image.new('RGBA',(self.w*S,self.h*S)); shadow=Image.new('RGBA',im.size); sd=ImageDraw.Draw(shadow)
        for _,pts,c in self.faces:
            if max(p[2] for p in pts)>4:
                pp=[]
                for p in pts:
                    x,y,z=self.rotate(p); pp.append(((self.o[0]+(x-y+z*.013)*32)*S,(self.o[1]+(x+y+z*.012)*16)*S))
                sd.polygon(pp,fill=(12,21,27,65))
        im.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(2.5*S)))
        # Per-pixel depth prevents tall walls and thin trim from incorrectly sorting through
        # each other. Each polygon is triangulated; illumination is baked into the RGBA sheet.
        rgba=np.zeros((self.h*S,self.w*S,4),dtype=np.uint8)
        depth=np.full((self.h*S,self.w*S),-1e9,dtype=np.float32)
        for _,pts,c in self.faces:
            pp=[self.project(p) for p in pts]
            zz=[sum(self.rotate(p)[:2])+p[2]/32 for p in pts]
            for i in range(1,len(pts)-1):
                inds=[0,i,i+1]; a,b,d=[pp[k] for k in inds]; za,zb,zd=[zz[k] for k in inds]
                lx=max(0,int(min(a[0],b[0],d[0])));hx=min(self.w*S,int(max(a[0],b[0],d[0]))+1)
                ly=max(0,int(min(a[1],b[1],d[1])));hy=min(self.h*S,int(max(a[1],b[1],d[1]))+1)
                if hx<=lx or hy<=ly:continue
                den=(b[1]-d[1])*(a[0]-d[0])+(d[0]-b[0])*(a[1]-d[1])
                if abs(den)<.00001:continue
                yy,xx=np.mgrid[ly:hy,lx:hx];xx=xx+.5;yy=yy+.5
                u=((b[1]-d[1])*(xx-d[0])+(d[0]-b[0])*(yy-d[1]))/den
                v=((d[1]-a[1])*(xx-d[0])+(a[0]-d[0])*(yy-d[1]))/den
                z=u*za+v*zb+(1-u-v)*zd
                inside=(u>=-.0001)&(v>=-.0001)&(u+v<=1.0001)&(z>=depth[ly:hy,lx:hx]-.00001)
                depth[ly:hy,lx:hx][inside]=z[inside]
                grain=((xx.astype(int)*17+yy.astype(int)*37)%11-5)*.26
                # Brushed metal and concrete variation, plus a broad soft reflection.
                grain+=1.4*np.sin(xx*.041+yy*.028)+.7*np.sin(yy*1.71)
                grain+=np.maximum(0,1-np.abs(xx/self.w/S-.38)*5)*2.5
                color=np.stack([np.clip(q+grain,0,255) for q in c]+[np.full(xx.shape,255)],axis=-1).astype(np.uint8)
                rgba[ly:hy,lx:hx][inside]=color[inside]
        # Baked contact occlusion in small recesses, track gaps and roof machinery.
        occupied=rgba[:,:,3]>0
        occlusion=np.zeros(depth.shape,dtype=np.float32)
        for dx,dy in [(4,0),(-4,0),(0,4),(0,-4),(8,3),(-8,-3),(3,8),(-3,-8)]:
            nearby=np.roll(depth,(dy,dx),(0,1))
            delta=nearby-depth
            occlusion+=((delta>.09)&(delta<.70)&occupied)*.042
        rgba[:,:,:3]=np.uint8(rgba[:,:,:3]*(1-occlusion[:,:,None]))
        im.alpha_composite(Image.fromarray(rgba))
        return im

def hall(m,x,y,w,d,h,team=BLUE,c=STEEL):
    m.box(x,y,3,w,d,h,c)
    m.box(x-.025,y-.025,h+3,w+.05,d+.05,3,team)
    m.box(x+.09,y+.09,h+6,w-.18,d-.18,1,DARK)
    for i in range(int(w*8)):
        u=x+.06+i*.12
        m.box(u,y+d+.005,4,.025,.025,h-2,shade(c,.7),False)
    for i in range(int(d*7)):
        v=y+.08+i*.13
        m.box(x+w,v,4,.02,.022,h-2,shade(c,.75),False)
    for i in range(int(w*3)):
        u=x+.12+i*.3
        m.box(u,y+d+.025,12,.17,.03,6,DARK)
        m.box(u+.015,y+d+.06,13,.14,.015,4,GLASS)
    for i in range(int(d*3)):
        v=y+.12+i*.3
        m.box(x+w+.025,v,12,.025,.18,6,DARK)
        m.box(x+w+.055,v+.015,13,.015,.15,4,GLASS)
    for i in range(int((w-.25)*5)):
        m.box(x+.12+i*.2,y+.12,h+7,.015,d-.24,.3,STEEL,False)
    # Air conditioning, intake grille, roof access hatch.
    if w>.7:
        m.box(x+.18,y+.18,h+7,.34,.3,4,STEEL)
        for i in range(5): m.box(x+.20+i*.055,y+.2,h+11,.015,.24,.4,DARK,False)
        m.box(x+w-.38,y+.15,h+7,.2,.3,1,WHITE)

def ring(m,x,y,z,r,c): m.cyl(x,y,z,r,2,c)
def coil(m,x,y,z,h,team):
    m.cyl(x,y,z,.18,h,DARK)
    for t in range(6): ring(m,x,y,z+4+t*(h-6)/6,.24,GOLD)
    m.ellipsoid(x,y,z+h,.25,.25,6,team)
def dish(m,x,y,z,r):
    m.cyl(x,y,z-12,.12,12,DARK)
    m.ellipsoid(x,y,z,r,r*.7,4,WHITE)
    for i in range(8):
        a=i*pi/4; m.beam((x,y,z+3),(x+r*cos(a),y+r*.7*sin(a),z+2),.4,STEEL)
    m.beam((x,y,z+3),(x-.13,y-.1,z+16),.8,WHITE)

# --- RA2-style faction architecture -----------------------------------------------------------
# Allied: clean white concrete, blue-grey cladding, glass and domes. Soviet: heavy concrete, brick,
# rusted corrugated steel and copper. Team paint (BLUE/RED) marks roof bands, emblems and trims.
A_CONC=(206,204,194); A_TRIM=(150,162,172); S_CONC=(128,124,112); BRICK=(128,66,50)
RUST=(116,74,54); COPPER=(184,110,60); GLOW=(150,220,236)
SOVIET_B={'tesla_reactor','nuclear_reactor','cloning_vats','tesla_coil','iron_curtain','nuclear_silo',
          'flak_cannon','sentry_gun','psychic_sensor','radar_tower','rocket_tower'}

def dome(m,x,y,z,rx,ry,rz,c,n=24,rings=6):
    m.begin(True)
    for j in range(rings):
        a=j*pi/2/rings; b=(j+1)*pi/2/rings
        for i in range(n):
            u=i*2*pi/n; v=(i+1)*2*pi/n
            pts=[(x+rx*cos(t)*cos(s),y+ry*cos(t)*sin(s),z+rz*sin(t)) for t,s in [(a,u),(a,v),(b,v),(b,u)]]
            m.face(pts[:3] if j==rings-1 else pts,c)
    m.end()

def lathe(m,x,y,z,prof,c,n=24,cap=None):
    """Surface of revolution through (radius cells, height px) pairs, bottom to top."""
    m.begin(True)
    for (r0,h0),(r1,h1) in zip(prof,prof[1:]):
        for i in range(n):
            a,b=i*2*pi/n,(i+1)*2*pi/n
            m.face([(x+r0*cos(a),y+r0*sin(a),z+h0),(x+r0*cos(b),y+r0*sin(b),z+h0),
                    (x+r1*cos(b),y+r1*sin(b),z+h1),(x+r1*cos(a),y+r1*sin(a),z+h1)],c)
    m.end()
    if cap:
        r,h=prof[-1]; m.face([(x+r*.97*cos(i*2*pi/n),y+r*.97*sin(i*2*pi/n),z+h-1.5) for i in range(n)],cap)

def vault(m,x0,x1,y0,y1,z,h,c,along='y',n=12):
    """Half-cylinder (quonset/hangar) roof over the rectangle, ridge running `along`."""
    m.begin(True)
    pts=[]
    for i in range(n+1):
        t=pi*i/n
        if along=='y': pts.append(((x0+x1)/2+(x1-x0)/2*cos(t),z+h*sin(t)))
        else: pts.append(((y0+y1)/2+(y1-y0)/2*cos(t),z+h*sin(t)))
    for (a,za),(b,zb) in zip(pts,pts[1:]):
        if along=='y': m.face([(a,y0,za),(b,y0,zb),(b,y1,zb),(a,y1,za)],c)
        else: m.face([(x0,a,za),(x0,b,zb),(x1,b,zb),(x1,a,za)],c)
    m.end()
    if along=='y':
        m.face([(a,y1,za) for a,za in pts],shade(c,.92)); m.face([(a,y0,za) for a,za in pts],shade(c,.92))
    else:
        m.face([(x1,a,za) for a,za in pts],shade(c,.92)); m.face([(x0,a,za) for a,za in pts],shade(c,.92))

def torus(m,x,y,z,R,r,c,n=28,vertical=False):
    """A ring of tube: horizontal (around z), or vertical in the x/z plane (an arch)."""
    for i in range(n):
        a,b=i*2*pi/n,(i+1)*2*pi/n
        if vertical: p=(x+R*cos(a),y,z+R*ZC*sin(a)); q=(x+R*cos(b),y,z+R*ZC*sin(b))
        else: p=(x+R*cos(a),y+R*sin(a),z); q=(x+R*cos(b),y+R*sin(b),z)
        m.tube(p,q,r,c,n=8,caps=False)

def lattice(m,x,y,z,h,r0,r1,c):
    """Tapering four-legged lattice mast with cross braces."""
    legs=[(-1,-1),(1,-1),(1,1),(-1,1)]
    for sx,sy in legs: m.tube((x+sx*r0,y+sy*r0,z),(x+sx*r1,y+sy*r1,z+h),.018,c,n=6)
    for k in range(5):
        t0,t1=k/5,(k+1)/5
        ra,rb=r0+(r1-r0)*t0,r0+(r1-r0)*t1
        for (ax,ay),(bx,by) in zip(legs,legs[1:]+legs[:1]):
            m.tube((x+ax*ra,y+ay*ra,z+h*t0),(x+bx*rb,y+by*rb,z+h*t1),.009,c,n=4,caps=False)

def star(m,cx,y,cz,R,c):
    """Five-pointed emblem on a +y facing wall at (cx, y), centre height cz."""
    pts=[]
    for k in range(10):
        a=k*pi/5; r=R if k%2==0 else R*.4
        pts.append((cx+r*sin(a),y,cz+r*ZC*cos(a)))
    m.face([pts[k] for k in range(1,10,2)],c)
    for k in range(0,10,2): m.face([pts[k-1],pts[k],pts[(k+1)%10]],c)

def star_x(m,x,cy,cz,R,c):
    pts=[]
    for k in range(10):
        a=k*pi/5; r=R if k%2==0 else R*.4
        pts.append((x,cy-r*sin(a),cz+r*ZC*cos(a)))
    m.face([pts[k] for k in range(1,10,2)],c)
    for k in range(0,10,2): m.face([pts[k-1],pts[k],pts[(k+1)%10]],c)

def flag(m,x,y,z,h,team):
    m.tube((x,y,z),(x,y,z+h),.012,STEEL,n=6)
    m.face([(x,y,z+h-1),(x+.28,y,z+h-2),(x+.28,y,z+h-8),(x,y,z+h-7)],team)

def sandbags(m,pts,c=(150,136,104)):
    for (x,y) in pts: m.ellipsoid(x,y,2,.07,.045,2.2,c)

def a_block(m,x,y,w,d,h,team,c=A_CONC,z=3,windows=True):
    """Allied block: white concrete, a recessed glass band, blue-grey plinth, team roof band."""
    m.box(x,y,z,w,d,h,c)
    m.box(x-.01,y-.01,z,w+.02,d+.02,4,A_TRIM)
    m.box(x-.012,y-.012,z+h-3,w+.024,d+.024,3,team)
    m.box(x+.04,y+.04,z+h,w-.08,d-.08,.8,shade(c,.86),False)
    if windows:
        m.box(x+.08,y+d,z+h*.45,w-.16,.012,h*.22,GLASS,False)
        m.box(x+w,y+.08,z+h*.45,.012,d-.16,h*.22,GLASS,False)

def s_block(m,x,y,w,d,h,team,z=3):
    """Soviet block: brick walls on a concrete plinth, concrete cornice, rusted roof, team band."""
    m.box(x,y,z,w,d,6,S_CONC)
    m.box(x+.01,y+.01,z+6,w-.02,d-.02,h-9,BRICK)
    for i in range(int(w*5)):  # pilasters
        m.box(x+.05+i*w/int(w*5),y+d-.005,z+6,.04,.02,h-9,shade(S_CONC,.95),False)
    for i in range(int(d*5)):
        m.box(x+w-.015,y+.05+i*d/int(d*5),z+6,.02,.04,h-9,shade(S_CONC,.95),False)
    m.box(x-.015,y-.015,z+h-3,w+.03,d+.03,3,S_CONC)
    m.box(x-.016,y+d+.001,z+h-2.4,w+.032,.012,1.4,team,False)
    m.box(x+w+.001,y-.016,z+h-2.4,.012,d+.032,1.4,team,False)
    m.box(x+.03,y+.03,z+h,w-.06,d-.06,.8,RUST,False)
    for i in range(int(w*3)):
        m.box(x+.1+i*w/max(1,int(w*3)),y+d,z+10,.09,.014,5,(60,62,58),False)

def smokestack(m,x,y,z,h,r,team):
    m.cyl(x,y,z,r,h,S_CONC,n=16,r2=r*.8)
    m.cyl(x,y,z+h*.8,r*.86,3,team,n=16)
    m.cyl(x,y,z+h,r*.8,1,DARK,n=16)

def slab(m,w,d,sov):
    base=(104,106,100) if sov else (150,150,142)
    m.box(.07,.07,0,w-.14,d-.14,3,base)
    for i in range(w*2):
        m.box(.1+i*.5,.1,3,.014,d-.2,.2,shade(base,.8),False)
    for j in range(d*2): m.box(.1,.1+j*.5,3,w-.2,.014,.2,shade(base,.8),False)
    for i in range(w*5):
        m.box(.14+i*.18,d-.15,3,.08,.065,.3,GOLD if i%2 else DARK,False)
    for x,y in [(.13,d-.22),(w-.22,.13),(w-.22,d-.22)]:
        m.box(x,y,3,.1,.1,2,DARK); m.box(x,y,5,.1,.1,.6,(184,224,212))


LEAF=(56,94,44); LEAF2=(78,116,50); BARK=(92,70,48); HAZARD=(214,178,40)

def building(ident,w,d):
    import hd_buildings
    m=Model((w+d)*32+32,(w+d)*16+96,(16+d*32,88))
    hd_buildings.build(m,ident,w,d)
    return m.render()

def unit(ident,facing):
    import hd_units
    m=Model(96,96,(48,70),facing*pi/4)
    hd_units.build(m,ident)
    return m.render()

WALK_FRAMES=6
def walkers():
    import hd_infantry
    return hd_infantry.WALKERS

def unit_walk(ident,pose,facing=0):
    """One frame of an infantry unit's walk cycle: a 64 px frame around the same ground point
    (the standing sheet's frame less its empty margin: 16 px each side, 24 above, 8 below)."""
    import hd_units,hd_infantry
    m=Model(64,64,(32,46),facing*pi/4)
    hd_infantry.POSE=pose
    try: hd_units.build(m,ident)
    finally: hd_infantry.POSE=None
    return m.render()

def main():
    """Flat Pillow previews of every model (seconds per sprite; no lighting or materials)."""
    import sys
    out=Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'target/hd-preview'
    (out/'buildings').mkdir(parents=True,exist_ok=True);(out/'units').mkdir(exist_ok=True)
    economy=(ROOT/'assets/data/economy.ron').read_text().split('    units:')[0]
    defs=re.findall(r'id: "([^"]+)".*?width: (\d+),\s*height: (\d+)',economy,re.S)
    for ident,w,h in defs:
        building(ident,int(w),int(h)).save(out/'buildings'/f'{ident}.png')
    units=re.findall(r'id: "([^"]+)"',(ROOT/'assets/data/units.ron').read_text())
    for ident in units:
        sheet=Image.new('RGBA',(96*S*8,96*S))
        for facing in range(8):sheet.alpha_composite(unit(ident,facing),(96*S*facing,0))
        sheet.save(out/'units'/f'{ident}.png')
    print(f'Previewed {len(defs)} buildings and {len(units)} eight-direction unit sheets in {out}.')
if __name__=='__main__': main()
