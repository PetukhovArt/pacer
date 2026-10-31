import { createApp } from 'vue';
import { VeyraClient } from '@core/client-sdk';
import { installClient } from '@core/vue-bindings';
import { installUiKit } from '@ui/kit';
import 'dockview-vue/dist/styles/dockview.css';
import './app.css';
import App from './App.vue';

const app = createApp(App);
installUiKit(app);
installClient(app, new VeyraClient(location.origin));
app.mount('#app');
