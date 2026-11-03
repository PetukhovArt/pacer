import { createApp } from 'vue';
import { VeyraClient } from '@core/client-sdk';
import { installClient } from '@core/vue-bindings';
import { installUiKit } from '@ui/kit';
import './app.css';
import App from './App.vue';

const app = createApp(App);
installUiKit(app);
const client = new VeyraClient(location.origin);
installClient(app, client);
void client.setWorkspace(localStorage.getItem('veyra.workspace'));
void client.restore().finally(() => app.mount('#app'));
