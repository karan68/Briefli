package com.briefli.companion

import android.app.Application
import com.briefli.companion.capture.CaptureRecovery
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class BriefliApplication : Application() {
	lateinit var container: AppContainer
		private set

	private val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

	override fun onCreate() {
		super.onCreate()
		container = AppContainer(this)
		applicationScope.launch {
			CaptureRecovery(container.captureRepository).recoverInterruptedCaptures()
		}
	}
}
