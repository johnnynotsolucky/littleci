<template>
	<v-card flat style="border-radius: 0;">
		<v-toolbar
			flat
			dark
			>
			<v-toolbar-title>
				Add New User
			</v-toolbar-title>
			<v-spacer />
				<v-toolbar-items>
					<v-btn @click="onSave">
						Save
						<v-icon right small>fas fa-save</v-icon>
					</v-btn>
				</v-toolbar-items>
		</v-toolbar>
		<v-container
			fluid
			class="grey lighten-4 fill-height"
			>
			<v-row>
				<v-col>
					{{ error }}
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						v-model="username"
						label="Username"
						outlined
						></v-text-field>
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						v-model="password"
						label="Password"
						type="password"
						outlined
						></v-text-field>
				</v-col>
			</v-row>
		</v-container>
	</v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state} from '../store/state'

@Component({
	name: 'NewUser',
})
export default class NewUser extends Vue {
	state = state

	username = ''
	password = ''

	error = ''

	async onSave() {
	try {
		await this.state.saveNewUser({
			username: this.username,
			password: this.password,
		})

		this.$router.replace('/manage/users')
	} catch (error) {
		this.error = error
	}
	}
}
</script>

