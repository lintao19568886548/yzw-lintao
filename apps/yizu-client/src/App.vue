<script setup lang="ts">
import { onLaunch } from '@dcloudio/uni-app'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'

onLaunch(() => {
  useAuthStore().hydrate()
  useDemandStore().hydrate()
})
</script>

<style src="./styles/global.css"></style>
