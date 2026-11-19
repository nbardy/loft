// deck_at(seconds) supplies a rigid local-to-world frame. Keep anchors in its local coordinates.
fn placed(deck:Frame, joint:Frame)->Frame {
  let a=deck.q;
  let b=joint.q;
  let q=vec4f(a.w*b.xyz+b.w*a.xyz+cross(a.xyz,b.xyz), a.w*b.w-dot(a.xyz,b.xyz));
  return Frame(vec4f(deck.t.xyz+rot(a,joint.t.xyz),1),q);
}
fn deck_site(deck:Frame, world:vec3f)->vec3f {
  return rot(vec4f(-deck.q.xyz,deck.q.w),world-deck.t.xyz);
}
