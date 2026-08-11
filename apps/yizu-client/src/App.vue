<script setup lang="ts">
import { onLaunch } from '@dcloudio/uni-app'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'

onLaunch(() => {
  useAuthStore().hydrate()
  useDemandStore().hydrate()
  useHistoryStore().hydrate()
})
</script>

<style src="./styles/global.css"></style>
