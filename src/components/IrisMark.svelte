<!--
  IrisMark : signe Iris (enveloppe en contour, rabat ouvert, trois traits d'aile de chaque côté).
  Props : size (number, px, défaut 32) · color (string CSS, défaut "currentColor") · title (string, optionnel, sinon décoratif).
  Doit rester identique à src/assets/iris-mark.svg.
-->
<script lang="ts">
  let { size = 32, color = "currentColor", title }: { size?: number; color?: string; title?: string } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" fill="none" stroke={color} stroke-width="2.6"
  stroke-linejoin="miter" stroke-linecap="butt" role={title ? "img" : undefined} aria-hidden={title ? undefined : true}
  aria-label={title}>
  <path d="M19 22H45V42H19Z" />
  <path d="M19 22L32 33L45 22" />
  <path d="M2 32H17M4 22L16 26M4 42L16 38" />
  <path d="M62 32H47M60 22L48 26M60 42L48 38" />
</svg>
